use chrono::prelude::*;
use polars::prelude::*;

fn main() {
    println!("Hello, r_proj_062!");

    let mut df: DataFrame = df!(
        "name" => ["Alice Archer", "Ben Brown", "Chloe Cooper", "Daniel Donovan"],
        "birthdate" => [
            NaiveDate::from_ymd_opt(1997, 1, 10).unwrap(),
            NaiveDate::from_ymd_opt(1995, 2, 10).unwrap(),
            NaiveDate::from_ymd_opt(1990, 10, 8).unwrap(),
            NaiveDate::from_ymd_opt(1992, 8,1).unwrap(),
        ],
        "weight" => [57.9, 72.5, 53.6, 83.1],
        "height" => [1.56, 1.77, 1.65, 1.75],
    ).unwrap();

    println!("{df}");

    use std::fs::File;

    let mut file = File::create("./table.csv").expect("Could not create file");
    CsvWriter::new(&mut file)
        .include_header(true)
        .with_separator(b',')
        .finish(&mut df)
        .expect("Could not write CSV file");

    let df_csv = CsvReadOptions::default()
        .with_has_header(true)
        .with_parse_options(CsvParseOptions::default().with_try_parse_dates(true))
        .try_into_reader_with_file_path(Some("./table.csv".into()))
        .expect("Could not open CSV file")
        .finish()
        .expect("Could not parse CSV file");
    println!("{df_csv}");

    let result = df.clone()
        .lazy()
        .select([
            col("name"),
            col("birthdate").dt().year().alias("birth_year"),
            (col("weight") / col("height").pow(2)).alias("bmi"),
        ])
        .collect();

    println!("{:?}", result);

    let res1 = df
        .clone()
        .lazy()
        .select([
            col("name"),
            (cols(["weight", "height"]).as_expr() * lit(0.95))
                .round(2, RoundMode::default())
                .name()
                .suffix("-5%"),
        ])
        .collect();
    println!("{:?}", res1);

    let res2 = df
        .clone()
        .lazy()
        .filter(col("birthdate").dt().year().lt(lit(1995)))
        .collect();
    println!("{:?}", res2);

    let res3 = df
        .clone()
        .lazy()
        .filter(
            col("birthdate")
                .is_between(
                    lit(NaiveDate::from_ymd_opt(1992, 1, 1).unwrap()),
                    lit(NaiveDate::from_ymd_opt(1995, 12, 31).unwrap()),
                    ClosedInterval::Both,
                )
                .and(col("height").gt(lit(1.7))),
        )
        .collect();
    println!("{:?}", res3);

    // Use `group_by_stable` if you want the Python behaviour of `maintain_order=True`.
    let res4 = df
        .clone()
        .lazy()
        .group_by([(col("birthdate").dt().year() / lit(10) * lit(10)).alias("decade")])
        .agg([len()])
        .collect();
    println!("{:?}", res4);

    let res5 = df
        .clone()
        .lazy()
        .group_by([(col("birthdate").dt().year() / lit(10) * lit(10)).alias("decade")])
        .agg([
            len().alias("sample_size"),
            col("weight")
                .mean()
                .round(2, RoundMode::default())
                .alias("avg_weight"),
            col("height").max().alias("tallest"),
        ])
        .collect();
    println!("{:?}",res5);

    let df2: DataFrame = df!(
        "name" => ["Ben Brown", "Daniel Donovan", "Alice Archer", "Chloe Cooper"],
        "parent" => [true, false, false, false],
        "siblings" => [1, 2, 3, 4],
        )
        .unwrap();

    let res6 = df
        .clone()
        .lazy()
        .join(
            df2.lazy(),
            [col("name")],
            [col("name")],
            JoinArgs::new(JoinType::Left),
        )
        .collect();

    println!("{:?}", res6);

    let df3: DataFrame = df!(
    "name" => ["Ethan Edwards", "Fiona Foster", "Grace Gibson", "Henry Harris"],
    "birthdate" => [
        NaiveDate::from_ymd_opt(1977, 5, 10).unwrap(),
        NaiveDate::from_ymd_opt(1975, 6, 23).unwrap(),
        NaiveDate::from_ymd_opt(1973, 7, 22).unwrap(),
        NaiveDate::from_ymd_opt(1971, 8, 3).unwrap(),
        ],
        "weight" => [67.9, 72.5, 57.6, 93.1],  // (kg)
        "height" => [1.76, 1.6, 1.66, 1.8],  // (m)
        )
        .unwrap();

    let res7 = concat([df.clone().lazy(), df3.lazy()], UnionArgs::default())
        .expect("Could not concatenate frames")
        .collect();
    println!("{:?}", res7);
}
