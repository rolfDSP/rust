//! A comprehensive tour of the Polars DataFrame library in Rust.
//!
//! Polars is a fast, multi-threaded DataFrame library built on the Apache
//! Arrow memory format. Its core concepts are:
//!
//!   * `Series`    – a single, typed, named column (a chunked Arrow array).
//!   * `DataFrame` – a collection of equally long `Series` (columns).
//!   * `Expr`      – a *description* of a computation on columns,
//!     e.g. `col("price") * lit(2)`. Expressions are composable
//!     and are executed in parallel by the query engine.
//!   * `LazyFrame` – a query plan. Nothing is computed until `.collect()`.
//!     Before execution the optimizer rewrites the plan
//!     (predicate / projection pushdown, common subexpression
//!     elimination, ...).
//!
//! ```text
//!   DataFrame ──.lazy()──► LazyFrame ──.filter/.select/.group_by/...──►
//!       ▲                                                              │
//!       └──────────────────────────── .collect() ◄─────────────────────┘
//! ```
//!
//! Rule of thumb: build DataFrames eagerly, but do the actual work with
//! the lazy expression API – it is the idiomatic and fastest way.
//!
//! Sections of this example:
//!   1.  Creating and inspecting DataFrames
//!   2.  Working with a single Series
//!   3.  Select, filter, with_columns (the expression API)
//!   4.  Conditional logic and string operations
//!   5.  Group by and aggregations
//!   6.  Window functions
//!   7.  Sorting, ranking and cumulative operations
//!   8.  Joins
//!   9.  Concatenation
//!  10.  Reshaping: pivot and unpivot
//!  11.  Missing data (nulls)
//!  12.  Dates, times and dynamic (time-based) group by
//!  13.  Nested data: lists and structs
//!  14.  I/O: CSV, Parquet, JSON, and lazy scanning with pushdown
//!  15.  SQL interface
//!  16.  Getting data back into plain Rust

use std::fs::File;
use std::sync::Arc;

use chrono::NaiveDate;
use polars::prelude::*;
use polars::sql::SQLContext;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CSV_PATH: &str = "sales.csv";
const PARQUET_PATH: &str = "sales.parquet";
const JSON_PATH: &str = "sales.json";

fn main() -> Result<()> {
    let sales = section_1_create_and_inspect()?;
    section_2_series()?;
    section_3_select_filter_with_columns(&sales)?;
    section_4_conditionals_and_strings(&sales)?;
    section_5_group_by(&sales)?;
    section_6_window_functions(&sales)?;
    section_7_sort_rank_cumulative(&sales)?;
    section_8_joins(&sales)?;
    section_9_concat()?;
    section_10_reshape(&sales)?;
    section_11_missing_data()?;
    section_12_temporal()?;
    section_13_nested_data()?;
    section_14_io(&sales)?;
    section_15_sql(&sales)?;
    section_16_back_to_rust(&sales)?;

    // Clean up the files we wrote.
    for p in [CSV_PATH, PARQUET_PATH, JSON_PATH] {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

fn header(title: &str) {
    println!("\n{}\n{title}\n{}", "=".repeat(78), "=".repeat(78));
}

// ---------------------------------------------------------------------------
// 1. Creating and inspecting DataFrames
// ---------------------------------------------------------------------------
fn section_1_create_and_inspect() -> Result<DataFrame> {
    header("1. Creating and inspecting DataFrames");

    // The `df!` macro is the quickest way to build a DataFrame.
    // Each column is "name" => slice/array/vec. `Option<T>` gives nulls.
    let sales = df!(
        "order_id" => [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        "date"     => [
            NaiveDate::from_ymd_opt(2026, 1, 3).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 9).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 22).unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 2).unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 8).unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 14).unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 7).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 19).unwrap(),
        ],
        "region"   => ["North", "South", "North", "East", "South", "East",
                       "North", "West", "South", "West", "East", "North"],
        "product"  => ["Laptop", "Phone", "Tablet", "Laptop", "Phone", "Tablet",
                       "Phone", "Laptop", "Laptop", "Phone", "Phone", "Tablet"],
        "quantity" => [2i32, 5, 1, 3, 4, 2, 6, 1, 2, 7, 3, 4],
        "price"    => [1200.0f64, 650.0, 400.0, 1150.0, 640.0, 420.0,
                       600.0, 1300.0, 1250.0, 610.0, 630.0, 390.0],
        "rep"      => [Some("Anna"), Some("Ben"), Some("Anna"), Some("Chen"),
                       None, Some("Chen"), Some("Dora"), Some("Eli"),
                       Some("Ben"), Some("Eli"), None, Some("Dora")],
    )?;

    // A DataFrame implements Display – printing shows a nice table.
    println!("{sales}");

    println!("shape         : {:?}  (rows, columns)", sales.shape());
    println!("height / width: {} / {}", sales.height(), sales.width());
    println!("column names  : {:?}", sales.get_column_names());
    println!("schema        :\n{:?}", sales.schema());

    // head / tail / slice are cheap (zero-copy views).
    println!("head(3):\n{}", sales.head(Some(3)));
    println!("slice(5, 2):\n{}", sales.slice(5, 2));

    // Quick summary statistics, computed with expressions.
    let summary = sales
        .clone()
        .lazy()
        .select([
            col("price").count().alias("count"),
            col("price").mean().alias("mean"),
            col("price").std(1).alias("std"),
            col("price").min().alias("min"),
            col("price").median().alias("median"),
            col("price").max().alias("max"),
        ])
        .collect()?;
    println!("price summary:\n{summary}");

    // A DataFrame can also be assembled from Series / Columns.
    let a = Column::new("a".into(), &[1i64, 2, 3]);
    let b = Column::new("b".into(), &["x", "y", "z"]);
    let small = DataFrame::new_infer_height(vec![a, b])?;
    println!("built from columns:\n{small}");

    Ok(sales)
}

// ---------------------------------------------------------------------------
// 2. Series
// ---------------------------------------------------------------------------
fn section_2_series() -> Result<()> {
    header("2. Working with a single Series");

    let s = Series::new("values".into(), &[Some(3.0f64), Some(1.5), None, Some(7.25), Some(4.0)]);
    println!("{s}");

    println!("dtype      : {}", s.dtype());
    println!("len        : {}", s.len());
    println!("null_count : {}", s.null_count());
    println!("sum        : {:?}", s.sum::<f64>()?);
    println!("mean       : {:?}", s.mean());
    println!("min / max  : {:?} / {:?}", s.min::<f64>()?, s.max::<f64>()?);

    // Arithmetic is vectorised; nulls propagate.
    let doubled = &s * 2.0;
    println!("s * 2:\n{doubled}");

    // Comparisons produce a boolean mask, which can filter the Series.
    let mask = s.gt(2.0)?;
    println!("s > 2 -> {:?}", s.filter(&mask)?);

    // Downcast to a typed ChunkedArray to iterate as Option<T>.
    let ca: &Float64Chunked = s.f64()?;
    let as_vec: Vec<Option<f64>> = ca.iter().collect();
    println!("as Vec<Option<f64>>: {as_vec:?}");

    // Casting between types.
    let as_int = s.cast(&DataType::Int32)?;
    println!("cast to Int32: {as_int}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Select, filter, with_columns
// ---------------------------------------------------------------------------
fn section_3_select_filter_with_columns(sales: &DataFrame) -> Result<()> {
    header("3. Select, filter, with_columns (the expression API)");

    // `.clone()` on a DataFrame is cheap: columns are reference counted.
    let out = sales
        .clone()
        .lazy()
        // with_columns adds or replaces columns, keeping all others.
        .with_columns([
            (col("quantity") * col("price")).alias("revenue"),
            col("product").str().to_uppercase().alias("product_uc"),
        ])
        // filter keeps rows where the boolean expression is true.
        .filter(
            col("revenue")
                .gt(lit(2000))
                .and(col("region").neq(lit("West"))),
        )
        // select keeps only the listed expressions (projection).
        .select([
            col("order_id"),
            col("region"),
            col("product_uc"),
            col("revenue"),
            // Expressions can be arbitrarily complex.
            (col("revenue") * lit(0.19)).round(2, RoundMode::HalfToEven).alias("vat"),
        ])
        .collect()?;
    println!("revenue > 2000 outside West:\n{out}");

    // Selecting multiple columns by regex or by dtype.
    let numeric = sales
        .clone()
        .lazy()
        .select([dtype_cols([DataType::Int32, DataType::Float64]).as_selector().as_expr()])
        .collect()?;
    println!("only Int32/Float64 columns (head 3):\n{}", numeric.head(Some(3)));

    let by_regex = sales
        .clone()
        .lazy()
        .select([col("^(order_id|pr.*)$")])
        .collect()?;
    println!("columns matching ^(order_id|pr.*)$ (head 3):\n{}", by_regex.head(Some(3)));

    // Excluding columns.
    let without = sales
        .clone()
        .lazy()
        .select([all().exclude_cols(["date", "rep"]).as_expr()])
        .collect()?;
    println!("all except date & rep (head 3):\n{}", without.head(Some(3)));

    // Eager equivalents also exist, e.g. drop / rename / filter with a mask.
    let mut renamed = sales.drop("rep")?;
    renamed.rename("quantity", "qty".into())?;
    println!("eager drop + rename (head 2):\n{}", renamed.head(Some(2)));

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Conditionals and strings
// ---------------------------------------------------------------------------
fn section_4_conditionals_and_strings(sales: &DataFrame) -> Result<()> {
    header("4. Conditional logic and string operations");

    let out = sales
        .clone()
        .lazy()
        .select([
            col("order_id"),
            col("product"),
            col("price"),
            // when / then / otherwise == SQL CASE WHEN
            when(col("price").gt_eq(lit(1000)))
                .then(lit("premium"))
                .when(col("price").gt_eq(lit(500)))
                .then(lit("standard"))
                .otherwise(lit("budget"))
                .alias("tier"),
            // String namespace
            col("product").str().len_chars().alias("name_len"),
            col("product").str().contains(lit("a"), false).alias("has_a"),
            col("product").str().slice(lit(0), lit(3)).alias("abbr"),
            // Combine strings
            concat_str([col("region"), col("product")], "-", true).alias("key"),
            // is_in: membership test against a list of values
            col("region")
                .is_in(lit(Series::new("".into(), ["North", "East"])).implode(false), false)
                .alias("north_or_east"),
        ])
        .collect()?;
    println!("{out}");

    // String replacement and extraction with regex.
    let codes = df!("code" => ["INV-2026-0012", "INV-2025-0450", "CRD-2026-0007"])?
        .lazy()
        .with_columns([
            col("code").str().extract(lit(r"^([A-Z]+)"), 1).alias("kind"),
            col("code")
                .str()
                .extract(lit(r"-(\d{4})-"), 1)
                .cast(DataType::Int32)
                .alias("year"),
            col("code")
                .str()
                .replace_all(lit("-"), lit("/"), true)
                .alias("slashed"),
        ])
        .collect()?;
    println!("regex extraction:\n{codes}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Group by
// ---------------------------------------------------------------------------
fn section_5_group_by(sales: &DataFrame) -> Result<()> {
    header("5. Group by and aggregations");

    let by_region = sales
        .clone()
        .lazy()
        .with_column((col("quantity") * col("price")).alias("revenue"))
        .group_by([col("region")])
        .agg([
            len().alias("orders"),
            col("quantity").sum().alias("units"),
            col("revenue").sum().alias("revenue"),
            col("revenue").mean().round(1, RoundMode::HalfToEven).alias("avg_order"),
            col("price").max().alias("max_price"),
            col("product").n_unique().alias("distinct_products"),
            // Aggregations can be filtered inside the group.
            col("revenue")
                .filter(col("product").eq(lit("Laptop")))
                .sum()
                .alias("laptop_revenue"),
            // Collect group values into a list.
            col("order_id").alias("order_ids"),
        ])
        // group_by does not preserve order – sort for deterministic output.
        .sort(["revenue"], SortMultipleOptions::default().with_order_descending(true))
        .collect()?;
    println!("by region:\n{by_region}");

    // Group by multiple keys; `group_by_stable` keeps first-appearance order.
    let by_region_product = sales
        .clone()
        .lazy()
        .group_by_stable([col("region"), col("product")])
        .agg([col("quantity").sum()])
        .collect()?;
    println!("by region & product (stable order):\n{by_region_product}");

    // Whole-frame aggregation without group_by.
    let totals = sales
        .clone()
        .lazy()
        .select([
            (col("quantity") * col("price")).sum().alias("total_revenue"),
            col("quantity").mean().alias("avg_qty"),
            col("price").std(1).alias("price_std"),
            col("price").quantile(lit(0.5), QuantileMethod::Linear).alias("price_median"),
        ])
        .collect()?;
    println!("totals:\n{totals}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Window functions
// ---------------------------------------------------------------------------
fn section_6_window_functions(sales: &DataFrame) -> Result<()> {
    header("6. Window functions");

    // `.over(partition)` computes an expression per group, but broadcasts the
    // result back to every row – like SQL `... OVER (PARTITION BY ...)`.
    let out = sales
        .clone()
        .lazy()
        .with_column((col("quantity") * col("price")).alias("revenue"))
        .select([
            col("order_id"),
            col("region"),
            col("revenue"),
            col("revenue").sum().over([col("region")])?.alias("region_total"),
            (col("revenue") / col("revenue").sum().over([col("region")])? * lit(100))
                .round(1, RoundMode::HalfToEven)
                .alias("pct_of_region"),
            col("revenue")
                .rank(RankOptions { method: RankMethod::Dense, descending: true }, None)
                .over([col("region")])?
                .alias("rank_in_region"),
            // Previous order's revenue in the same region (lag).
            col("revenue").shift(lit(1)).over([col("region")])?.alias("prev_in_region"),
        ])
        .sort(["region", "order_id"], Default::default())
        .collect()?;
    println!("{out}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Sorting, ranking, cumulative
// ---------------------------------------------------------------------------
fn section_7_sort_rank_cumulative(sales: &DataFrame) -> Result<()> {
    header("7. Sorting, ranking and cumulative operations");

    // Multi-column sort with per-column direction.
    let sorted = sales
        .clone()
        .lazy()
        .select([col("region"), col("product"), col("price")])
        .sort(
            ["region", "price"],
            SortMultipleOptions::default()
                .with_order_descending_multi([false, true])
                .with_nulls_last(true),
        )
        .collect()?;
    println!("sorted by region asc, price desc:\n{sorted}");

    // Top-k per group: sort inside the group and take the first n.
    let top2 = sales
        .clone()
        .lazy()
        .group_by([col("region")])
        .agg([col("price")
            .sort(SortOptions::default().with_order_descending(true))
            .head(Some(2))
            .alias("top2_prices")])
        .sort(["region"], Default::default())
        .collect()?;
    println!("top-2 prices per region:\n{top2}");

    // Running totals over the (date-ordered) frame.
    let cumulative = sales
        .clone()
        .lazy()
        .sort(["date"], Default::default())
        .select([
            col("date"),
            col("quantity"),
            col("quantity").cum_sum(false).alias("running_units"),
            col("price").cum_max(false).alias("running_max_price"),
            col("quantity")
                .rolling_mean(RollingOptionsFixedWindow {
                    window_size: 3,
                    min_periods: 1,
                    ..Default::default()
                })
                .alias("units_ma3"),
            col("quantity").diff(lit(1), Default::default()).alias("diff"),
        ])
        .collect()?;
    println!("cumulative / rolling:\n{cumulative}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Joins
// ---------------------------------------------------------------------------
fn section_8_joins(sales: &DataFrame) -> Result<()> {
    header("8. Joins");

    let products = df!(
        "product"  => ["Laptop", "Phone", "Tablet", "Watch"],
        "category" => ["Computers", "Mobile", "Mobile", "Wearables"],
        "cost"     => [800.0, 350.0, 220.0, 120.0],
    )?;

    let reps = df!(
        "rep"  => ["Anna", "Ben", "Chen", "Dora", "Fritz"],
        "team" => ["A", "A", "B", "B", "C"],
    )?;

    // Inner join on one key, then compute margin.
    let joined = sales
        .clone()
        .lazy()
        .join(
            products.clone().lazy(),
            [col("product")],
            [col("product")],
            JoinArgs::new(JoinType::Inner),
        )
        .with_column(((col("price") - col("cost")) * col("quantity")).alias("margin"))
        .select([col("order_id"), col("product"), col("category"), col("margin")])
        .collect()?;
    println!("inner join with products:\n{}", joined.head(Some(5)));

    // Left join keeps every sales row; unmatched rows get nulls.
    let left = sales
        .clone()
        .lazy()
        .left_join(reps.clone().lazy(), col("rep"), col("rep"))
        .select([col("order_id"), col("rep"), col("team")])
        .collect()?;
    println!("left join with reps:\n{left}");

    // Full outer join: also reps without any sales (Fritz) appear.
    let rep_sales = sales
        .clone()
        .lazy()
        .group_by([col("rep")])
        .agg([len().alias("orders")]);
    let full = rep_sales
        .join(
            reps.clone().lazy(),
            [col("rep")],
            [col("rep")],
            JoinArgs::new(JoinType::Full).with_coalesce(JoinCoalesce::CoalesceColumns),
        )
        .sort(["rep"], SortMultipleOptions::default().with_nulls_last(true))
        .collect()?;
    println!("full outer join (coalesced keys):\n{full}");

    // Semi join: products that were sold at least once. Anti: never sold.
    let sold = products
        .clone()
        .lazy()
        .join(
            sales.clone().lazy(),
            [col("product")],
            [col("product")],
            JoinArgs::new(JoinType::Semi),
        )
        .collect()?;
    let unsold = products
        .clone()
        .lazy()
        .join(
            sales.clone().lazy(),
            [col("product")],
            [col("product")],
            JoinArgs::new(JoinType::Anti),
        )
        .collect()?;
    println!("semi join (sold products):\n{sold}");
    println!("anti join (never sold):\n{unsold}");

    // Cross join: every combination (cartesian product).
    let sizes = df!("size" => ["S", "M"])?;
    let cross = products
        .clone()
        .lazy()
        .select([col("product")])
        .cross_join(sizes.lazy(), None)
        .collect()?;
    println!("cross join ({} rows):\n{}", cross.height(), cross.head(Some(4)));

    Ok(())
}

// ---------------------------------------------------------------------------
// 9. Concatenation
// ---------------------------------------------------------------------------
fn section_9_concat() -> Result<()> {
    header("9. Concatenation");

    let jan = df!("month" => ["Jan", "Jan"], "units" => [10, 20])?;
    let feb = df!("month" => ["Feb"], "units" => [30])?;

    // Vertical: stack rows (schemas must match).
    let vertical = concat([jan.clone().lazy(), feb.lazy()], UnionArgs::default())?.collect()?;
    println!("vertical:\n{vertical}");

    // Diagonal: union of all columns, missing ones are filled with null.
    let mar = df!("month" => ["Mar"], "units" => [40], "returns" => [2])?;
    let diagonal = concat_lf_diagonal([vertical.lazy(), mar.lazy()], UnionArgs::default())?
        .collect()?;
    println!("diagonal:\n{diagonal}");

    // Horizontal: put columns side by side (heights must match).
    let extra = df!("target" => [15, 15])?;
    let horizontal = jan.hstack(extra.columns())?;
    println!("horizontal:\n{horizontal}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 10. Reshaping
// ---------------------------------------------------------------------------
fn section_10_reshape(sales: &DataFrame) -> Result<()> {
    header("10. Reshaping: pivot and unpivot");

    // Pivot (long -> wide): one row per region, one column per product.
    // The lazy pivot needs to know the output columns up front, so we pass
    // the distinct values of the `on` column. Inside the aggregation,
    // `element()` refers to the values of each (region, product) cell.
    let on_values = sales
        .clone()
        .lazy()
        .select([col("product").unique().sort(Default::default())])
        .collect()?;
    let pivoted = sales
        .clone()
        .lazy()
        .pivot(
            by_name(["product"], true, false),
            Arc::new(on_values),
            by_name(["region"], true, false),
            by_name(["quantity"], true, false),
            element().sum(),
            true,
            "_".into(),
            polars::frame::PivotColumnNaming::Auto,
        )
        .sort(["region"], Default::default())
        .collect()?;
    println!("pivot (sum of quantity):\n{pivoted}");

    // Unpivot (wide -> long) – the inverse operation ("melt").
    let long = pivoted
        .lazy()
        .unpivot(UnpivotArgsDSL {
            index: by_name(["region"], true, false),
            on: Some(by_name(["Laptop", "Phone", "Tablet"], true, false)),
            variable_name: Some("product".into()),
            value_name: Some("quantity".into()),
        })
        .drop_nulls(None)
        .sort(["region", "product"], Default::default())
        .collect()?;
    println!("unpivot back to long format:\n{long}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 11. Missing data
// ---------------------------------------------------------------------------
fn section_11_missing_data() -> Result<()> {
    header("11. Missing data (nulls)");

    let df = df!(
        "sensor" => ["a", "a", "a", "b", "b", "b"],
        "temp"   => [Some(20.5), None, Some(21.5), None, Some(18.0), None],
        "note"   => [Some("ok"), None, None, Some("check"), None, None],
    )?;
    println!("{df}");

    // null_count per column.
    println!("null counts:\n{}", df.null_count());

    let filled = df
        .clone()
        .lazy()
        .with_columns([
            col("temp").fill_null(lit(0.0)).alias("temp_zero"),
            col("temp")
                .fill_null_with_strategy(FillNullStrategy::Forward(None))
                .over([col("sensor")])?
                .alias("temp_ffill"),
            col("temp")
                .fill_null(col("temp").mean().over([col("sensor")])?)
                .alias("temp_mean_fill"),
            col("temp").interpolate(InterpolationMethod::Linear).alias("temp_interp"),
            col("note").fill_null(lit("-")),
            col("temp").is_null().alias("was_missing"),
        ])
        .collect()?;
    println!("filled:\n{filled}");

    // Drop rows that have a null in a given column subset.
    let dropped = df
        .lazy()
        .drop_nulls(Some(by_name(["temp"], true, false)))
        .collect()?;
    println!("drop_nulls on temp:\n{dropped}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 12. Temporal data
// ---------------------------------------------------------------------------
fn section_12_temporal() -> Result<()> {
    header("12. Dates, times and dynamic (time-based) group by");

    // Parse strings into Datetime.
    let events = df!(
        "ts"    => ["2026-11-01 08:15:00", "2026-11-01 09:45:00", "2026-11-01 11:05:00",
                    "2026-11-02 08:30:00", "2026-11-02 13:20:00", "2026-11-03 10:00:00"],
        "value" => [3, 7, 2, 5, 8, 4],
    )?
    .lazy()
    .with_column(col("ts").str().to_datetime(
        Some(TimeUnit::Milliseconds),
        None,
        StrptimeOptions {
            format: Some("%Y-%m-%d %H:%M:%S".into()),
            ..Default::default()
        },
        lit("raise"),
    ))
    .collect()?;
    println!("parsed:\n{events}");

    // dt() namespace: extract components, format, arithmetic.
    let parts = events
        .clone()
        .lazy()
        .select([
            col("ts"),
            col("ts").dt().date().alias("date"),
            col("ts").dt().hour().alias("hour"),
            col("ts").dt().weekday().alias("weekday"),
            col("ts").dt().strftime("%d.%m.%Y %H:%M").alias("german_fmt"),
            col("ts").dt().truncate(lit("1h")).alias("hour_bucket"),
            (col("ts") - col("ts").first()).alias("since_first"),
        ])
        .collect()?;
    println!("datetime components:\n{parts}");

    // group_by_dynamic: aggregate into fixed time windows.
    let daily = events
        .clone()
        .lazy()
        .sort(["ts"], Default::default())
        .group_by_dynamic(
            col("ts"),
            [],
            DynamicGroupOptions {
                every: Duration::parse("1d"),
                period: Duration::parse("1d"),
                offset: Duration::parse("0ns"),
                ..Default::default()
            },
        )
        .agg([len().alias("events"), col("value").sum().alias("total")])
        .collect()?;
    println!("daily windows (group_by_dynamic):\n{daily}");

    // Filtering by a date range.
    let start = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let recent = events
        .lazy()
        .filter(col("ts").gt_eq(lit(start)))
        .collect()?;
    println!("events since 2026-11-02:\n{recent}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 13. Nested data
// ---------------------------------------------------------------------------
fn section_13_nested_data() -> Result<()> {
    header("13. Nested data: lists and structs");

    let orders = df!(
        "customer" => ["Ann", "Bob", "Cid"],
        "tags"     => ["new,vip", "returning", "vip,eu,b2b"],
    )?;

    // Split strings into List[str], then operate on the lists.
    let lists = orders
        .clone()
        .lazy()
        .with_column(col("tags").str().split(lit(",")))
        .with_columns([
            col("tags").list().len().alias("n_tags"),
            col("tags").list().first().alias("first_tag"),
            col("tags")
                .list()
                .contains(lit("vip"), false)
                .alias("is_vip"),
            col("tags")
                .list()
                .eval(col("").str().to_uppercase())
                .alias("upper_tags"),
        ])
        .collect()?;
    println!("list operations:\n{lists}");

    // explode: one row per list element.
    let exploded = lists
        .clone()
        .lazy()
        .select([col("customer"), col("tags")])
        .explode(by_name(["tags"], true, false), ExplodeOptions { empty_as_null: true, keep_nulls: true })
        .collect()?;
    println!("exploded:\n{exploded}");

    // Structs bundle several fields into one column.
    let structs = orders
        .lazy()
        .select([
            as_struct(vec![col("customer"), col("tags")]).alias("record"),
        ])
        .collect()?;
    println!("struct column:\n{structs}");

    let unnested = structs
        .clone()
        .lazy()
        .select([col("record").struct_().field_by_name("customer")])
        .collect()?;
    println!("field 'customer' of the struct:\n{unnested}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 14. I/O
// ---------------------------------------------------------------------------
fn section_14_io(sales: &DataFrame) -> Result<()> {
    header("14. I/O: CSV, Parquet, JSON and lazy scanning with pushdown");

    let mut df = sales.clone();

    // --- CSV -------------------------------------------------------------
    CsvWriter::new(File::create(CSV_PATH)?)
        .include_header(true)
        .with_separator(b',')
        .finish(&mut df)?;
    let from_csv = CsvReadOptions::default()
        .with_has_header(true)
        .map_parse_options(|o| o.with_try_parse_dates(true))
        .try_into_reader_with_file_path(Some(CSV_PATH.into()))?
        .finish()?;
    println!("read back from CSV (schema): {:?}", from_csv.schema());

    // --- Parquet ---------------------------------------------------------
    ParquetWriter::new(File::create(PARQUET_PATH)?)
        .with_compression(ParquetCompression::Zstd(None))
        .with_statistics(StatisticsOptions::full())
        .finish(&mut df)?;
    let from_parquet = ParquetReader::new(File::open(PARQUET_PATH)?).finish()?;
    println!(
        "parquet round trip equal: {}",
        from_parquet.equals_missing(sales)
    );

    // --- JSON ------------------------------------------------------------
    JsonWriter::new(File::create(JSON_PATH)?)
        .with_json_format(JsonFormat::Json)
        .finish(&mut df)?;
    let from_json = JsonReader::new(File::open(JSON_PATH)?).finish()?;
    println!("read back from JSON (head 2):\n{}", from_json.head(Some(2)));

    // --- Lazy scanning ---------------------------------------------------
    // scan_* creates a LazyFrame without reading the file. The optimizer
    // pushes the filter and the column selection *into the reader*, so only
    // needed columns (and, for Parquet, row groups) are read.
    let lf = LazyFrame::scan_parquet(PARQUET_PATH.into(), ScanArgsParquet::default())?
        .filter(col("region").eq(lit("North")))
        .group_by([col("product")])
        .agg([col("quantity").sum()])
        .sort(["product"], Default::default());

    println!("optimized query plan:\n{}", lf.explain(true)?);
    println!("result:\n{}", lf.collect()?);

    let csv_lazy = LazyCsvReader::new(CSV_PATH.into())
        .with_has_header(true)
        .finish()?
        .select([col("product"), col("price")])
        .filter(col("price").lt(lit(500)))
        .collect()?;
    println!("lazy CSV scan (price < 500):\n{csv_lazy}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 15. SQL
// ---------------------------------------------------------------------------
fn section_15_sql(sales: &DataFrame) -> Result<()> {
    header("15. SQL interface");

    let mut ctx = SQLContext::new();
    ctx.register("sales", sales.clone().lazy());

    let result = ctx
        .execute(
            "SELECT region,
                    COUNT(*)                     AS orders,
                    SUM(quantity * price)        AS revenue,
                    ROUND(AVG(price), 2)         AS avg_price
             FROM sales
             WHERE product <> 'Tablet'
             GROUP BY region
             HAVING SUM(quantity) > 5
             ORDER BY revenue DESC",
        )?
        .collect()?;
    println!("{result}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 16. Back to Rust
// ---------------------------------------------------------------------------
fn section_16_back_to_rust(sales: &DataFrame) -> Result<()> {
    header("16. Getting data back into plain Rust");

    // A single scalar from a one-row aggregation.
    let total: f64 = sales
        .clone()
        .lazy()
        .select([(col("quantity") * col("price")).sum().alias("revenue")])
        .collect()?
        .column("revenue")?
        .f64()?
        .get(0)
        .unwrap_or_default();
    println!("total revenue as f64: {total:.2}");

    // Column -> Vec.
    let regions: Vec<Option<&str>> = sales.column("region")?.str()?.iter().collect();
    println!("regions: {regions:?}");

    // Zip typed columns to build Rust structs.
    struct Order<'a> {
        id: u32,
        product: &'a str,
        quantity: i32,
        rep: Option<&'a str>,
    }

    let ids = sales.column("order_id")?.u32()?;
    let products = sales.column("product")?.str()?;
    let quantities = sales.column("quantity")?.i32()?;
    let reps = sales.column("rep")?.str()?;

    let orders: Vec<Order> = ids
        .iter()
        .zip(products.iter())
        .zip(quantities.iter())
        .zip(reps.iter())
        .filter_map(|(((id, product), quantity), rep)| {
            Some(Order { id: id?, product: product?, quantity: quantity?, rep })
        })
        .take(3)
        .collect();
    for o in &orders {
        println!(
            "order #{:<2} {:>2} x {:<7} sold by {}",
            o.id,
            o.quantity,
            o.product,
            o.rep.unwrap_or("<unknown>")
        );
    }

    // Generic row access (slower – prefer the columnar approach above).
    let row = sales.get_row(0)?;
    println!("row 0 as AnyValues: {:?}", row.0);

    Ok(())
}
