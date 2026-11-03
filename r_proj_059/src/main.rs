//! A comprehensive tour of the Apache Parquet file format in Rust.
//!
//! Parquet is a *columnar* storage format: instead of storing row after row,
//! it stores all values of one column together. That gives
//!   * excellent compression (similar values sit next to each other),
//!   * fast analytics (read only the columns you need = "projection"),
//!   * skipping of data using min/max statistics ("predicate pushdown").
//!
//! File layout (simplified):
//!
//! ```text
//! +-------------------------------------------+
//! | "PAR1" magic                              |
//! | Row group 0                               |
//! |   Column chunk "id"     -> pages ...      |
//! |   Column chunk "city"   -> pages ...      |
//! |   ...                                     |
//! | Row group 1                               |
//! |   ...                                     |
//! | Footer: schema, row-group metadata,       |
//! |         column statistics, key/values     |
//! | footer length + "PAR1" magic              |
//! +-------------------------------------------+
//! ```
//!
//! The Rust `parquet` crate offers two APIs:
//!   1. The **Arrow API** (`parquet::arrow`) – reads/writes Apache Arrow
//!      `RecordBatch`es. This is what you want 95% of the time.
//!   2. The **low-level API** (`parquet::file`, `parquet::record`) – direct
//!      access to file metadata, row groups, and a row-by-row iterator.
//!
//! This example demonstrates both.

use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, ListBuilder, RecordBatch,
    StringArray, StringBuilder, TimestampMillisecondArray,
};
use arrow::compute::kernels::cmp::gt;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::util::pretty::print_batches;

use parquet::arrow::ArrowWriter;
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{ArrowPredicateFn, ParquetRecordBatchReaderBuilder, RowFilter};
use parquet::basic::{Compression, Encoding, ZstdLevel};
use parquet::file::metadata::KeyValue;
use parquet::file::properties::{EnabledStatistics, WriterProperties, WriterVersion};
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::statistics::Statistics;
use parquet::record::RowAccessor;
use parquet::schema::types::ColumnPath;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const FILE_PATH: &str = "weather.parquet";
const TOTAL_ROWS: usize = 10_000;
const ROWS_PER_BATCH: usize = 2_500;
const ROWS_PER_ROW_GROUP: usize = 2_000;

fn main() -> Result<()> {
    println!("=== 1. Writing a Parquet file ===");
    write_parquet(FILE_PATH)?;

    println!("\n=== 2. Inspecting file metadata (low-level API) ===");
    inspect_metadata(FILE_PATH)?;

    println!("\n=== 3. Reading the whole file as Arrow RecordBatches ===");
    read_all(FILE_PATH)?;

    println!("\n=== 4. Projection: reading only some columns ===");
    read_projection(FILE_PATH)?;

    println!("\n=== 5. Row-group pruning using column statistics ===");
    read_with_row_group_pruning(FILE_PATH, 4_200, 4_210)?;

    println!("\n=== 6. Row filter (predicate pushdown during decoding) ===");
    read_with_row_filter(FILE_PATH, 38.0)?;

    println!("\n=== 7. Row-by-row access with the record API ===");
    read_rows_low_level(FILE_PATH, 3)?;

    println!("\n=== 8. A small analytic query: average temperature per city ===");
    average_temperature_per_city(FILE_PATH)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Schema & sample data
// ---------------------------------------------------------------------------

/// The Arrow schema of our data set. It is converted to a Parquet schema
/// automatically by the `ArrowWriter` (and embedded in the file so that the
/// exact Arrow types are restored on reading).
fn weather_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("city", DataType::Utf8, false),
        // nullable column: a sensor may fail and deliver no value
        Field::new("temperature", DataType::Float64, true),
        Field::new("is_raining", DataType::Boolean, false),
        Field::new(
            "recorded_at",
            DataType::Timestamp(TimeUnit::Millisecond, Some("+00:00".into())),
            false,
        ),
        // nested column: a list of strings (Parquet supports nested data!)
        Field::new(
            "tags",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
    ]))
}

/// Generate a deterministic batch of fake weather measurements.
fn make_batch(schema: &SchemaRef, start: usize, len: usize) -> Result<RecordBatch> {
    const CITIES: [&str; 5] = ["Berlin", "Munich", "Hamburg", "Stuttgart", "Cologne"];
    const BASE_TS: i64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z in ms

    let ids: Int64Array = (start..start + len).map(|i| i as i64).collect();

    let cities: StringArray = (start..start + len)
        .map(|i| Some(CITIES[i % CITIES.len()]))
        .collect();

    // Every 97th measurement is missing -> NULL
    let temperatures: Float64Array = (start..start + len)
        .map(|i| {
            if i % 97 == 0 {
                None
            } else {
                // pseudo-random-ish value between -10 and +40
                Some((((i * 7919) % 500) as i64 - 100) as f64 / 10.0)
            }
        })
        .collect();

    let raining: BooleanArray = (start..start + len).map(|i| Some(i % 3 == 0)).collect();

    let timestamps = TimestampMillisecondArray::from_iter_values(
        (start..start + len).map(|i| BASE_TS + i as i64 * 60_000), // one per minute
    )
    .with_timezone("+00:00");

    let mut tags = ListBuilder::new(StringBuilder::new());
    for i in start..start + len {
        match i % 4 {
            0 => tags.append_null(),
            1 => {
                tags.values().append_value("calibrated");
                tags.append(true);
            }
            2 => {
                tags.values().append_value("outdoor");
                tags.values().append_value("rooftop");
                tags.append(true);
            }
            _ => tags.append(true), // empty list (not null!)
        }
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(ids),
        Arc::new(cities),
        Arc::new(temperatures),
        Arc::new(raining),
        Arc::new(timestamps),
        Arc::new(tags.finish()),
    ];
    Ok(RecordBatch::try_new(schema.clone(), columns)?)
}

// ---------------------------------------------------------------------------
// 1. Writing
// ---------------------------------------------------------------------------

fn write_parquet(path: &str) -> Result<()> {
    let schema = weather_schema();

    // WriterProperties control *how* the data is laid out on disk.
    let props = WriterProperties::builder()
        .set_writer_version(WriterVersion::PARQUET_2_0)
        // default compression for all columns
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3)?))
        // rows per row group (the unit of parallelism and of skipping)
        .set_max_row_group_row_count(Some(ROWS_PER_ROW_GROUP))
        // collect min/max statistics per page (and therefore per chunk)
        .set_statistics_enabled(EnabledStatistics::Page)
        // "city" has few distinct values -> dictionary encoding is ideal
        .set_column_dictionary_enabled(ColumnPath::from("city"), true)
        // monotonic ids compress very well with delta encoding
        .set_column_dictionary_enabled(ColumnPath::from("id"), false)
        .set_column_encoding(ColumnPath::from("id"), Encoding::DELTA_BINARY_PACKED)
        // per-column compression override
        .set_column_compression(ColumnPath::from("is_raining"), Compression::SNAPPY)
        // bloom filter: allows fast "is value X definitely NOT in this row group?"
        .set_column_bloom_filter_enabled(ColumnPath::from("city"), true)
        .set_created_by("r_proj_059 parquet example".to_string())
        // arbitrary application metadata stored in the footer
        .set_key_value_metadata(Some(vec![
            KeyValue::new("source".to_string(), "fake weather sensors".to_string()),
            KeyValue::new("units.temperature".to_string(), "celsius".to_string()),
        ]))
        .build();

    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;

    // Write in several batches. The writer buffers data and cuts row groups
    // of `max_row_group_size` rows independently of the batch boundaries.
    for start in (0..TOTAL_ROWS).step_by(ROWS_PER_BATCH) {
        let len = ROWS_PER_BATCH.min(TOTAL_ROWS - start);
        let batch = make_batch(&schema, start, len)?;
        writer.write(&batch)?;
    }

    // `close` flushes the last row group and writes the footer. Without it
    // the file is NOT a valid Parquet file!
    let metadata = writer.close()?;
    println!(
        "wrote {} rows in {} row groups to '{path}' ({} bytes)",
        metadata.file_metadata().num_rows(),
        metadata.num_row_groups(),
        std::fs::metadata(path)?.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Metadata
// ---------------------------------------------------------------------------

fn inspect_metadata(path: &str) -> Result<()> {
    let reader = SerializedFileReader::new(File::open(path)?)?;
    let meta = reader.metadata();
    let file_meta = meta.file_metadata();

    println!("version   : {}", file_meta.version());
    println!("created_by: {}", file_meta.created_by().unwrap_or("?"));
    println!("num_rows  : {}", file_meta.num_rows());

    if let Some(kvs) = file_meta.key_value_metadata() {
        for kv in kvs {
            // the ARROW:schema entry is a large base64 blob – skip it
            if kv.key != "ARROW:schema" {
                println!("kv        : {} = {:?}", kv.key, kv.value);
            }
        }
    }

    println!("\nParquet schema:");
    parquet::schema::printer::print_schema(&mut std::io::stdout(), file_meta.schema());

    println!();
    for (rg_idx, rg) in meta.row_groups().iter().enumerate() {
        println!(
            "row group {rg_idx}: {} rows, {} bytes (uncompressed)",
            rg.num_rows(),
            rg.total_byte_size()
        );
        // Only show details of the first row group to keep output short
        if rg_idx > 0 {
            continue;
        }
        for col in rg.columns() {
            println!(
                "   {:<15} {:<7} comp={:>6}B uncomp={:>6}B encodings={:?} stats={}",
                col.column_path().string(),
                // Note: the codec is stored in the file, the compression *level* is not
                format!("{:?}", col.compression()).split('(').next().unwrap_or("?"),
                col.compressed_size(),
                col.uncompressed_size(),
                col.encodings().collect::<Vec<_>>(),
                format_stats(col.statistics()),
            );
        }
    }
    Ok(())
}

fn format_stats(stats: Option<&Statistics>) -> String {
    let Some(stats) = stats else {
        return "-".into();
    };
    let nulls = stats.null_count_opt().unwrap_or(0);
    match stats {
        Statistics::Int64(s) => format!("[{:?}..{:?}] nulls={nulls}", s.min_opt(), s.max_opt()),
        Statistics::Double(s) => format!("[{:?}..{:?}] nulls={nulls}", s.min_opt(), s.max_opt()),
        Statistics::Boolean(s) => format!("[{:?}..{:?}] nulls={nulls}", s.min_opt(), s.max_opt()),
        Statistics::ByteArray(s) => format!(
            "[{:?}..{:?}] nulls={nulls}",
            s.min_opt().and_then(|v| v.as_utf8().ok()),
            s.max_opt().and_then(|v| v.as_utf8().ok())
        ),
        other => format!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 3. Read everything
// ---------------------------------------------------------------------------

fn read_all(path: &str) -> Result<()> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;

    // The original Arrow schema (incl. timezone, list types) is restored.
    println!("arrow schema ({} fields):", builder.schema().fields().len());
    for f in builder.schema().fields() {
        println!("   {}: {} (nullable={})", f.name(), f.data_type(), f.is_nullable());
    }

    let reader = builder.with_batch_size(1024).build()?;

    let mut batches = 0;
    let mut rows = 0;
    let mut first: Option<RecordBatch> = None;
    for batch in reader {
        let batch = batch?;
        batches += 1;
        rows += batch.num_rows();
        if first.is_none() {
            first = Some(batch);
        }
    }
    println!("read {rows} rows in {batches} batches; first 5 rows:");
    if let Some(b) = first {
        print_batches(&[b.slice(0, 5)])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Projection
// ---------------------------------------------------------------------------

fn read_projection(path: &str) -> Result<()> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;

    // Select columns by *root* index in the Parquet schema: city(1), temperature(2).
    // Only these column chunks are read & decompressed from disk.
    let mask = ProjectionMask::roots(builder.parquet_schema(), [1, 2]);
    let reader = builder.with_projection(mask).with_batch_size(4).build()?;

    let first = reader.into_iter().next().transpose()?.expect("non-empty file");
    print_batches(&[first])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Row-group pruning
// ---------------------------------------------------------------------------

/// Read only rows with `lo <= id <= hi`, skipping whole row groups whose
/// min/max statistics prove they cannot contain matching rows.
fn read_with_row_group_pruning(path: &str, lo: i64, hi: i64) -> Result<()> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    let meta = builder.metadata().clone();

    // Column index of "id" among the parquet leaf columns
    let id_col = builder
        .parquet_schema()
        .columns()
        .iter()
        .position(|c| c.name() == "id")
        .expect("id column");

    let matching: Vec<usize> = meta
        .row_groups()
        .iter()
        .enumerate()
        .filter(|(_, rg)| match rg.column(id_col).statistics() {
            Some(Statistics::Int64(s)) => match (s.min_opt(), s.max_opt()) {
                (Some(&min), Some(&max)) => max >= lo && min <= hi,
                _ => true, // no stats -> must read it
            },
            _ => true,
        })
        .map(|(i, _)| i)
        .collect();

    println!(
        "row groups total={}, to read={matching:?}",
        meta.num_row_groups()
    );

    let reader = builder.with_row_groups(matching).build()?;

    let mut result = Vec::new();
    for batch in reader {
        let batch = batch?;
        // Filter the remaining rows exactly (row groups are coarse-grained)
        let ids = batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap();
        let mask: BooleanArray = ids.iter().map(|v| v.map(|v| v >= lo && v <= hi)).collect();
        let filtered = arrow::compute::filter_record_batch(&batch, &mask)?;
        if filtered.num_rows() > 0 {
            result.push(filtered);
        }
    }
    print_batches(&result)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Row filter
// ---------------------------------------------------------------------------

/// Use a `RowFilter`: the predicate column ("temperature") is decoded first,
/// and the other columns are only decoded for rows where the predicate is true.
fn read_with_row_filter(path: &str, min_temp: f64) -> Result<()> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    let schema_descr = builder.parquet_schema();

    // The predicate receives a batch containing only the projected columns.
    let predicate_mask = ProjectionMask::roots(schema_descr, [2]); // temperature
    let threshold = Float64Array::new_scalar(min_temp);
    let predicate = ArrowPredicateFn::new(predicate_mask, move |batch: RecordBatch| {
        // `gt` returns NULL for NULL input; NULL is treated as "false" (row dropped)
        gt(batch.column(0), &threshold)
    });

    // Output only id, city, temperature, recorded_at
    let output_mask = ProjectionMask::roots(schema_descr, [0, 1, 2, 4]);

    let reader = builder
        .with_projection(output_mask)
        .with_row_filter(RowFilter::new(vec![Box::new(predicate)]))
        .build()?;

    let batches: Vec<RecordBatch> = reader.collect::<std::result::Result<_, _>>()?;
    let total: usize = batches.iter().map(|b| b.num_rows()).sum();
    println!("{total} rows with temperature > {min_temp}; first 5:");
    if let Some(b) = batches.first() {
        print_batches(&[b.slice(0, b.num_rows().min(5))])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Low-level record API
// ---------------------------------------------------------------------------

fn read_rows_low_level(path: &str, n: usize) -> Result<()> {
    let reader = SerializedFileReader::new(File::open(path)?)?;

    // `get_row_iter(None)` iterates over all rows of all row groups as
    // dynamically typed `Row` values. Convenient, but much slower than Arrow.
    for row in reader.get_row_iter(None)?.take(n) {
        let row = row?;
        println!("{row}");
        // Access individual fields by position / typed getters:
        let id = row.get_long(0)?;
        let city = row.get_string(1)?;
        println!("   -> id={id}, city={city}");
    }

    // Reading a single row group:
    let rg = reader.get_row_group(1)?;
    println!("row group 1 has {} rows", rg.metadata().num_rows());
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Analytic query
// ---------------------------------------------------------------------------

fn average_temperature_per_city(path: &str) -> Result<()> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    let mask = ProjectionMask::roots(builder.parquet_schema(), [1, 2]); // city, temperature
    let reader = builder.with_projection(mask).build()?;

    // city -> (sum, count)
    let mut acc: HashMap<String, (f64, usize)> = HashMap::new();
    for batch in reader {
        let batch = batch?;
        let cities = batch.column(0).as_any().downcast_ref::<StringArray>().unwrap();
        let temps = batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap();
        for i in 0..batch.num_rows() {
            if temps.is_null(i) {
                continue;
            }
            let e = acc.entry(cities.value(i).to_string()).or_default();
            e.0 += temps.value(i);
            e.1 += 1;
        }
    }

    let mut cities: Vec<_> = acc.into_iter().collect();
    cities.sort_by(|a, b| a.0.cmp(&b.0));
    for (city, (sum, count)) in cities {
        println!("   {city:<10} avg={:>6.2} °C  (n={count})", sum / count as f64);
    }
    Ok(())
}
