//! Creates (if not existing) an SQLite database `people.db` with a table
//! `person` and inserts 10 rows of random data.
//!
//! Columns:
//!   Name     – String  (TEXT)
//!   Birthday – Date    (TEXT, ISO-8601 "YYYY-MM-DD", SQLite's date format)
//!   Street   – String  (TEXT)
//!   Village  – String  (TEXT)
//!   Number   – Integer (INTEGER)
//!   Value    – Float   (REAL)

use chrono::{Duration, NaiveDate};
use rand::Rng;
use rand::seq::IndexedRandom;
use rusqlite::{Connection, Result, params};

const DB_FILE: &str = "people.db";

const FIRST_NAMES: &[&str] = &[
    "Anna", "Ben", "Clara", "David", "Emma", "Felix", "Greta", "Hannes", "Ida", "Jonas",
    "Lena", "Max", "Nora", "Paul", "Sophie", "Tim",
];
const LAST_NAMES: &[&str] = &[
    "Müller", "Schmidt", "Schneider", "Fischer", "Weber", "Meyer", "Wagner", "Becker",
    "Schulz", "Hoffmann",
];
const STREETS: &[&str] = &[
    "Hauptstraße", "Schulstraße", "Gartenweg", "Bergstraße", "Lindenallee", "Kirchgasse",
    "Bahnhofstraße", "Mühlweg",
];
const VILLAGES: &[&str] = &[
    "Ahornfeld", "Birkenau", "Eichstetten", "Waldheim", "Seedorf", "Talhausen", "Wiesenbach",
];

/// One row of the `person` table.
#[derive(Debug)]
struct Person {
    id: i64,
    name: String,
    birthday: NaiveDate,
    street: String,
    village: String,
    number: i64,
    value: f64,
}

fn create_table(conn: &Connection) -> Result<()> {
    // IF NOT EXISTS: an existing database/table is reused, not overwritten.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS person (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            Name     TEXT    NOT NULL,
            Birthday TEXT    NOT NULL,   -- stored as 'YYYY-MM-DD'
            Street   TEXT    NOT NULL,
            Village  TEXT    NOT NULL,
            Number   INTEGER NOT NULL,
            Value    REAL    NOT NULL
        )",
        [],
    )?;
    Ok(())
}

/// Random date between 1940-01-01 and 2010-12-31.
fn random_birthday(rng: &mut impl Rng) -> NaiveDate {
    let start = NaiveDate::from_ymd_opt(1940, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2010, 12, 31).unwrap();
    let days = (end - start).num_days();
    start + Duration::days(rng.random_range(0..=days))
}

fn insert_random_rows(conn: &mut Connection, count: usize) -> Result<()> {
    let mut rng = rand::rng();

    // A transaction makes the 10 inserts atomic (and fast).
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO person (Name, Birthday, Street, Village, Number, Value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;

        for _ in 0..count {
            let name = format!(
                "{} {}",
                FIRST_NAMES.choose(&mut rng).unwrap(),
                LAST_NAMES.choose(&mut rng).unwrap()
            );
            let birthday = random_birthday(&mut rng);
            let street = STREETS.choose(&mut rng).unwrap().to_string();
            let village = VILLAGES.choose(&mut rng).unwrap().to_string();
            let number: i64 = rng.random_range(1..=200);
            // Random value with two decimal places, 0.00 .. 10000.00
            let value: f64 = (rng.random_range(0.0..10_000.0_f64) * 100.0).round() / 100.0;

            stmt.execute(params![name, birthday, street, village, number, value])?;
        }
    }
    tx.commit()
}

/// Converts one result row (column order: id, Name, Birthday, Street,
/// Village, Number, Value) into a `Person`.
fn row_to_person(row: &rusqlite::Row) -> Result<Person> {
    Ok(Person {
        id: row.get(0)?,
        name: row.get(1)?,
        birthday: row.get(2)?,
        street: row.get(3)?,
        village: row.get(4)?,
        number: row.get(5)?,
        value: row.get(6)?,
    })
}

fn read_all(conn: &Connection) -> Result<Vec<Person>> {
    let mut stmt = conn.prepare(
        "SELECT id, Name, Birthday, Street, Village, Number, Value
         FROM person ORDER BY id",
    )?;
    let rows = stmt.query_map([], row_to_person)?;
    rows.collect()
}

/// Returns all people whose `Value` is greater than `threshold`,
/// sorted by `Value` in descending order.
fn people_with_value_above(conn: &Connection, threshold: f64) -> Result<Vec<Person>> {
    let mut stmt = conn.prepare(
        "SELECT id, Name, Birthday, Street, Village, Number, Value
         FROM person
         WHERE Value > ?1
         ORDER BY Value DESC",
    )?;
    let rows = stmt.query_map(params![threshold], row_to_person)?;
    rows.collect()
}

/// Prints a list of people as a table.
fn print_people(people: &[Person]) {
    println!(
        "{:>4}  {:<18} {:<10}  {:<14} {:<12} {:>6} {:>10}",
        "ID", "Name", "Birthday", "Street", "Village", "Number", "Value"
    );
    println!("{}", "-".repeat(82));
    for p in people {
        println!(
            "{:>4}  {:<18} {:<10}  {:<14} {:<12} {:>6} {:>10.2}",
            p.id, p.name, p.birthday, p.street, p.village, p.number, p.value
        );
    }
}

/// Lists all entries of the `person` table and prints them to the console.
fn list_all(conn: &Connection) -> Result<()> {
    let people = read_all(conn)?;

    if people.is_empty() {
        println!("The database contains no entries.");
        return Ok(());
    }

    print_people(&people);
    println!("\nTotal entries in database: {}", people.len());
    Ok(())
}

fn main() -> Result<()> {
    // Opens the file, or creates it if it does not exist yet.
    let mut conn = Connection::open(DB_FILE)?;
    create_table(&conn)?;

    insert_random_rows(&mut conn, 10)?;
    println!("Inserted 10 random rows into '{DB_FILE}'.\n");

    list_all(&conn)?;

    let threshold = 5000.0;
    let rich = people_with_value_above(&conn, threshold)?;
    println!("\nPeople with a Value above {threshold:.2}:\n");
    if rich.is_empty() {
        println!("No entries found.");
    } else {
        print_people(&rich);
        println!("\nFound {} entries.", rich.len());
    }
    Ok(())
}
