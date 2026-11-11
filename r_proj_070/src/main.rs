//! A comprehensive SQLite example in Rust with the `rusqlite` crate.
//!
//! The program builds a small library database (authors, books, members and
//! loans) and walks through the features that are used most in practice:
//!
//!  1. Opening a database and setting the important PRAGMAs
//!  2. Schema migrations with `PRAGMA user_version`
//!  3. Inserting rows (positional and named parameters, `RETURNING`)
//!  4. Querying rows into Rust structs, `Option` for NULL, custom types
//!  5. UPDATE, DELETE and UPSERT
//!  6. Transactions with business rules, rollback and savepoints
//!  7. Handling constraint errors (UNIQUE, FOREIGN KEY, CHECK, NOT NULL)
//!  8. Reports: JOIN, GROUP BY, window functions, recursive CTE, views
//!  9. JSON columns
//! 10. Full-text search with FTS5
//! 11. BLOBs
//! 12. User-defined SQL functions written in Rust
//! 13. Bulk inserts: why transactions matter for speed
//! 14. Several threads writing to one database
//! 15. Indexes and `EXPLAIN QUERY PLAN`
//! 16. Backup, in-memory databases and `VACUUM INTO`
//!
//! Run it with `cargo run`. See README.md and DOCUMENTATION.md.

use std::fmt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use rusqlite::backup::Progress;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, Type, ValueRef};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Row, TransactionBehavior, ffi, named_params, params,
    params_from_iter,
};

const DB_PATH: &str = "library.db";
const BACKUP_PATH: &str = "library-backup.db";
const VACUUM_PATH: &str = "library-compact.db";

/// The maximum number of books a member may have on loan at the same time.
const MAX_OPEN_LOANS: i64 = 3;

fn main() -> Result<()> {
    // Start from an empty database so that every run prints the same output.
    for path in [DB_PATH, BACKUP_PATH, VACUUM_PATH] {
        remove_db_files(path)?;
    }

    let mut conn = open_database(DB_PATH)?;
    run_migrations(&mut conn)?;
    seed_data(&mut conn)?;
    queries(&conn)?;
    updates_and_upserts(&conn)?;
    transactions(&mut conn)?;
    constraint_errors(&conn)?;
    reports(&conn)?;
    json_columns(&conn)?;
    full_text_search(&conn)?;
    blobs(&conn)?;
    user_defined_functions(&conn)?;
    bulk_inserts(&mut conn)?;
    multiple_threads(&conn)?;
    query_plans(&conn)?;
    backup_and_memory(&conn)?;

    // `Connection` closes itself when it is dropped. `close()` does the same
    // but returns the error instead of ignoring it.
    conn.close().map_err(|(_conn, err)| err)?;
    println!("\nDone. The database is in '{DB_PATH}', open it with: sqlite3 {DB_PATH}");
    Ok(())
}

// ===========================================================================
// 1. Opening a database
// ===========================================================================

fn open_database(path: &str) -> Result<Connection> {
    heading(1, "Opening the database");

    // `open` creates the file if it does not exist yet.
    // `Connection::open_in_memory()` gives a database that lives only in RAM.
    let conn = Connection::open(path).with_context(|| format!("cannot open {path}"))?;
    configure(&conn)?;

    let version: String = conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
    let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    let fk: bool = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    println!("SQLite version {version}, file '{path}'");
    println!("journal_mode = {journal}, foreign_keys = {fk}");
    Ok(conn)
}

/// Settings that every connection to the database should use.
/// PRAGMAs (except `journal_mode`) are per connection, not stored in the file.
fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;    -- readers do not block the writer and vice versa
         PRAGMA synchronous = NORMAL;  -- safe with WAL and much faster than FULL
         PRAGMA foreign_keys = ON;     -- OFF by default for backwards compatibility!",
    )?;
    // If another connection holds the write lock, wait up to 5 s instead of
    // failing at once with SQLITE_BUSY ("database is locked").
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

// ===========================================================================
// 2. Schema migrations
// ===========================================================================

/// Each migration moves the schema one version forward. Never change a
/// migration that has already been shipped; add a new one instead.
const MIGRATIONS: &[&str] = &[
    // Version 1: the core tables.
    // STRICT tables reject values of the wrong type (normal SQLite tables
    // would happily store the text 'abc' in an INTEGER column).
    "CREATE TABLE authors (
         id      INTEGER PRIMARY KEY,          -- alias for the rowid, auto-assigned
         name    TEXT    NOT NULL UNIQUE,
         born    INTEGER,                      -- NULL = unknown
         country TEXT
     ) STRICT;

     CREATE TABLE books (
         id          INTEGER PRIMARY KEY,
         author_id   INTEGER NOT NULL REFERENCES authors(id) ON DELETE RESTRICT,
         title       TEXT    NOT NULL,
         isbn        TEXT    NOT NULL UNIQUE,
         genre       TEXT    NOT NULL
                     CHECK (genre IN ('novel', 'science', 'history', 'poetry', 'children')),
         year        INTEGER NOT NULL CHECK (year BETWEEN 1400 AND 2100),
         price_cents INTEGER NOT NULL CHECK (price_cents >= 0),
         copies      INTEGER NOT NULL DEFAULT 1 CHECK (copies >= 0),
         tags        TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(tags)),
         summary     TEXT    NOT NULL DEFAULT ''
     ) STRICT;
     CREATE INDEX idx_books_author ON books(author_id);

     CREATE TABLE members (
         id     INTEGER PRIMARY KEY,
         name   TEXT NOT NULL,
         email  TEXT NOT NULL UNIQUE COLLATE NOCASE,   -- 'A@x.org' = 'a@x.org'
         joined TEXT NOT NULL DEFAULT (date('now'))
     ) STRICT;

     CREATE TABLE loans (
         id          INTEGER PRIMARY KEY,
         book_id     INTEGER NOT NULL REFERENCES books(id)   ON DELETE CASCADE,
         member_id   INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
         loaned_on   TEXT NOT NULL,                -- ISO 8601 dates: 'YYYY-MM-DD'
         due_on      TEXT NOT NULL,
         returned_on TEXT,                         -- NULL = still on loan
         CHECK (due_on >= loaned_on)
     ) STRICT;
     CREATE INDEX idx_loans_member ON loans(member_id);
     -- A partial index: only the (few) open loans are indexed.
     CREATE INDEX idx_loans_open ON loans(book_id) WHERE returned_on IS NULL;",
    // Version 2: covers, full-text search and a view.
    "CREATE TABLE covers (
         book_id INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
         mime    TEXT NOT NULL,
         data    BLOB NOT NULL
     ) STRICT;

     -- An FTS5 index over books.title and books.summary. 'content=books'
     -- means the text itself is not copied, only the index is stored.
     CREATE VIRTUAL TABLE books_fts USING fts5(
         title, summary,
         content = 'books', content_rowid = 'id',
         tokenize = 'porter unicode61'          -- 'travels' finds 'travel'
     );
     -- Index the rows that already exist ...
     INSERT INTO books_fts(books_fts) VALUES ('rebuild');
     -- ... and keep the index in sync with triggers.
     CREATE TRIGGER books_fts_insert AFTER INSERT ON books BEGIN
         INSERT INTO books_fts(rowid, title, summary) VALUES (new.id, new.title, new.summary);
     END;
     CREATE TRIGGER books_fts_delete AFTER DELETE ON books BEGIN
         INSERT INTO books_fts(books_fts, rowid, title, summary)
         VALUES ('delete', old.id, old.title, old.summary);
     END;
     CREATE TRIGGER books_fts_update AFTER UPDATE OF title, summary ON books BEGIN
         INSERT INTO books_fts(books_fts, rowid, title, summary)
         VALUES ('delete', old.id, old.title, old.summary);
         INSERT INTO books_fts(rowid, title, summary) VALUES (new.id, new.title, new.summary);
     END;

     CREATE VIEW open_loans AS
     SELECT l.id, b.title, m.name AS member, l.loaned_on, l.due_on,
            CAST(julianday('now', 'start of day') - julianday(l.due_on) AS INTEGER)
                AS days_overdue
     FROM loans l
     JOIN books b   ON b.id = l.book_id
     JOIN members m ON m.id = l.member_id
     WHERE l.returned_on IS NULL;",
    // Version 3: schema evolution on a table that already has data.
    "ALTER TABLE members ADD COLUMN phone TEXT;

     CREATE TABLE page_views (
         id        INTEGER PRIMARY KEY,
         book_id   INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
         worker    INTEGER NOT NULL,
         viewed_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
     ) STRICT;",
];

fn run_migrations(conn: &mut Connection) -> Result<()> {
    heading(2, "Schema migrations");
    migrate(conn)?;
    // Running it again does nothing: the database knows its version.
    migrate(conn)?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<()> {
    // user_version is an integer in the database header that SQLite does not
    // use itself; it is free for the application.
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let target = MIGRATIONS.len() as i64;
    if current == target {
        println!("schema is up to date (version {current})");
        return Ok(());
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as i64 + 1;
        // DDL is transactional in SQLite: a failing migration leaves no
        // half-created tables behind.
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("migration to version {version} failed"))?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        println!("migrated schema to version {version}");
    }
    Ok(())
}

// ===========================================================================
// Domain types
// ===========================================================================

/// A Rust enum stored as TEXT. Implementing `ToSql` and `FromSql` lets it be
/// used directly as a parameter and with `row.get()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Genre {
    Novel,
    Science,
    History,
    Poetry,
    Children,
}

impl Genre {
    fn as_str(self) -> &'static str {
        match self {
            Genre::Novel => "novel",
            Genre::Science => "science",
            Genre::History => "history",
            Genre::Poetry => "poetry",
            Genre::Children => "children",
        }
    }
}

impl ToSql for Genre {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for Genre {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "novel" => Ok(Genre::Novel),
            "science" => Ok(Genre::Science),
            "history" => Ok(Genre::History),
            "poetry" => Ok(Genre::Poetry),
            "children" => Ok(Genre::Children),
            other => Err(FromSqlError::Other(
                format!("unknown genre '{other}'").into(),
            )),
        }
    }
}

#[derive(Debug)]
struct Author {
    name: String,
    born: Option<i64>, // NULL in the database <-> None in Rust
    country: Option<String>,
}

#[derive(Debug)]
struct Book {
    id: i64,
    author: String,
    title: String,
    genre: Genre,
    year: i64,
    price_cents: i64,
    copies: i64,
    tags: Vec<String>,
}

impl Book {
    /// One SELECT shared by all queries that return `Book`s.
    const SELECT: &'static str = "SELECT b.id, a.name AS author, b.title, b.genre, b.year,
                b.price_cents, b.copies, b.tags
         FROM books b JOIN authors a ON a.id = b.author_id";

    /// Maps one result row to a `Book`. Columns are read by name here;
    /// reading by index (`row.get(0)`) is a little faster.
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Book> {
        let tags_json: String = row.get("tags")?;
        let tags = serde_json::from_str(&tags_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, Type::Text, Box::new(e)))?;
        Ok(Book {
            id: row.get("id")?,
            author: row.get("author")?,
            title: row.get("title")?,
            genre: row.get("genre")?,
            year: row.get("year")?,
            price_cents: row.get("price_cents")?,
            copies: row.get("copies")?,
            tags,
        })
    }
}

// ===========================================================================
// 3. Inserting data
// ===========================================================================

struct NewBook {
    author: &'static str,
    title: &'static str,
    isbn: &'static str,
    genre: Genre,
    year: i64,
    price_cents: i64,
    copies: i64,
    tags: &'static [&'static str],
    summary: &'static str,
}

fn seed_data(conn: &mut Connection) -> Result<()> {
    heading(3, "Inserting data");

    // All seed rows in one transaction: either everything or nothing.
    let tx = conn.transaction()?;

    // --- a) Prepared statement + positional parameters + RETURNING ---------
    let authors: [(&str, Option<i64>, Option<&str>); 8] = [
        ("Jane Austen", Some(1775), Some("England")),
        ("Charles Darwin", Some(1809), Some("England")),
        ("Mary Shelley", Some(1797), Some("England")),
        ("Emily Dickinson", Some(1830), Some("USA")),
        ("Lewis Carroll", Some(1832), Some("England")),
        ("Homer", None, Some("Greece")), // birth year unknown -> NULL
        ("Jules Verne", Some(1828), Some("France")),
        ("Edward Gibbon", Some(1737), None),
    ];
    {
        // A statement is compiled once and executed many times. Values are
        // always bound as parameters, never formatted into the SQL string:
        // that prevents SQL injection and quoting bugs.
        let mut stmt = tx.prepare(
            "INSERT INTO authors (name, born, country) VALUES (?1, ?2, ?3) RETURNING id",
        )?;
        for (name, born, country) in authors {
            let id: i64 = stmt.query_row(params![name, born, country], |row| row.get(0))?;
            println!("author #{id}: {name}");
        }
    } // the statement borrows `tx`, so it must be dropped before `commit`

    // --- b) Named parameters ------------------------------------------------
    let books = seed_books();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO books (author_id, title, isbn, genre, year, price_cents, copies, tags, summary)
             VALUES ((SELECT id FROM authors WHERE name = :author),
                     :title, :isbn, :genre, :year, :price, :copies, :tags, :summary)",
        )?;
        for book in &books {
            stmt.execute(named_params! {
                ":author": book.author,
                ":title": book.title,
                ":isbn": book.isbn,
                ":genre": book.genre,             // our ToSql implementation
                ":year": book.year,
                ":price": book.price_cents,
                ":copies": book.copies,
                ":tags": serde_json::to_string(book.tags)?,
                ":summary": book.summary,
            })?;
        }
        println!("inserted {} books", books.len());
    }

    // --- c) execute() + last_insert_rowid() ---------------------------------
    for (name, email) in [
        ("Alice Smith", "alice@example.org"),
        ("Bob Jones", "bob@example.org"),
        ("Carla Diaz", "carla@example.org"),
        ("David Lee", "david@example.org"),
    ] {
        tx.execute(
            "INSERT INTO members (name, email) VALUES (?1, ?2)",
            (name, email),
        )?;
        println!("member #{}: {name}", tx.last_insert_rowid());
    }

    // --- d) Loans with dates relative to today --------------------------------
    // date('now', '-20 days') is computed by SQLite. A NULL modifier gives a
    // NULL date, which we use for "not returned yet".
    let loans: [(&str, &str, &str, &str, Option<&str>); 6] = [
        (
            "Frankenstein",
            "alice@example.org",
            "-20 days",
            "-6 days",
            None,
        ),
        (
            "On the Origin of Species",
            "bob@example.org",
            "-10 days",
            "+4 days",
            None,
        ),
        (
            "Emma",
            "carla@example.org",
            "-30 days",
            "-16 days",
            Some("-18 days"),
        ),
        (
            "Alice's Adventures in Wonderland",
            "alice@example.org",
            "-5 days",
            "+9 days",
            None,
        ),
        (
            "Pride and Prejudice",
            "bob@example.org",
            "-3 days",
            "+11 days",
            Some("-1 days"),
        ),
        (
            "The Odyssey",
            "carla@example.org",
            "-2 days",
            "+12 days",
            None,
        ),
    ];
    for (title, email, loaned, due, returned) in loans {
        tx.execute(
            "INSERT INTO loans (book_id, member_id, loaned_on, due_on, returned_on)
             VALUES (?1, ?2, date('now', ?3), date('now', ?4), date('now', ?5))",
            params![
                book_id(&tx, title)?,
                member_id(&tx, email)?,
                loaned,
                due,
                returned
            ],
        )?;
    }
    println!("inserted {} loans", loans.len());

    tx.commit()?;
    Ok(())
}

fn seed_books() -> Vec<NewBook> {
    use Genre::*;
    #[rustfmt::skip]
    let books = vec![
        NewBook { author: "Jane Austen", title: "Pride and Prejudice", isbn: "978-0-00-000001-1",
            genre: Novel, year: 1813, price_cents: 899, copies: 3,
            tags: &["classic", "romance", "england"],
            summary: "Elizabeth Bennet and the proud Mr Darcy overcome their first impressions \
                      in a story of manners, marriage and money." },
        NewBook { author: "Jane Austen", title: "Emma", isbn: "978-0-00-000002-8",
            genre: Novel, year: 1815, price_cents: 799, copies: 1,
            tags: &["classic", "romance"],
            summary: "A young woman who loves matchmaking learns that she misjudges the \
                      people around her." },
        NewBook { author: "Charles Darwin", title: "On the Origin of Species", isbn: "978-0-00-000003-5",
            genre: Science, year: 1859, price_cents: 1299, copies: 2,
            tags: &["classic", "biology", "evolution"],
            summary: "Darwin explains evolution by natural selection, drawing on observations \
                      from his travels and from the breeding of animals." },
        NewBook { author: "Charles Darwin", title: "The Voyage of the Beagle", isbn: "978-0-00-000004-2",
            genre: Science, year: 1839, price_cents: 1099, copies: 1,
            tags: &["travel", "biology"],
            summary: "A journal of a five-year voyage around the world, full of geology, \
                      animals and the people of South America." },
        NewBook { author: "Mary Shelley", title: "Frankenstein", isbn: "978-0-00-000005-9",
            genre: Novel, year: 1818, price_cents: 699, copies: 2,
            tags: &["classic", "horror", "science-fiction"],
            summary: "A young scientist creates a living creature and is haunted by the \
                      monster he abandoned." },
        NewBook { author: "Emily Dickinson", title: "Poems", isbn: "978-0-00-000006-6",
            genre: Poetry, year: 1890, price_cents: 599, copies: 1,
            tags: &["classic", "poetry"],
            summary: "Short poems about death, nature and the soul, published after the \
                      poet's death." },
        NewBook { author: "Lewis Carroll", title: "Alice's Adventures in Wonderland", isbn: "978-0-00-000007-3",
            genre: Children, year: 1865, price_cents: 549, copies: 2,
            tags: &["classic", "fantasy"],
            summary: "Alice follows a white rabbit down a hole into a world of curious \
                      creatures and nonsense." },
        NewBook { author: "Lewis Carroll", title: "Through the Looking-Glass", isbn: "978-0-00-000008-0",
            genre: Children, year: 1871, price_cents: 549, copies: 1,
            tags: &["fantasy"],
            summary: "Alice climbs through a mirror and plays her way across a giant chessboard." },
        NewBook { author: "Homer", title: "The Odyssey", isbn: "978-0-00-000009-7",
            genre: Poetry, year: 1614, price_cents: 999, copies: 1,
            tags: &["classic", "epic", "travel"],
            summary: "Odysseus travels for ten years after the Trojan War before he finally \
                      returns home to Ithaca." },
        NewBook { author: "Jules Verne", title: "Around the World in Eighty Days", isbn: "978-0-00-000010-3",
            genre: Novel, year: 1872, price_cents: 749, copies: 2,
            tags: &["adventure", "travel"],
            summary: "Phileas Fogg bets that he can travel around the world in eighty days." },
        NewBook { author: "Edward Gibbon", title: "The History of the Decline and Fall of the Roman Empire",
            isbn: "978-0-00-000011-0", genre: History, year: 1776, price_cents: 2499, copies: 1,
            tags: &["history", "rome"],
            summary: "A history of the Roman Empire from its height to the fall of Constantinople." },
    ];
    books
}

/// Looks up a book id by title. `prepare_cached` keeps the compiled
/// statement in a per-connection cache, which helps for small helper queries
/// that are called again and again.
fn book_id(conn: &Connection, title: &str) -> Result<i64> {
    let mut stmt = conn.prepare_cached("SELECT id FROM books WHERE title = ?1")?;
    stmt.query_row([title], |row| row.get(0))
        .optional()?
        .with_context(|| format!("no book titled '{title}'"))
}

fn member_id(conn: &Connection, email: &str) -> Result<i64> {
    let mut stmt = conn.prepare_cached("SELECT id FROM members WHERE email = ?1")?;
    stmt.query_row([email], |row| row.get(0))
        .optional()?
        .with_context(|| format!("no member with e-mail '{email}'"))
}

// ===========================================================================
// 4. Queries
// ===========================================================================

fn queries(conn: &Connection) -> Result<()> {
    heading(4, "Queries");

    // --- a) A single value --------------------------------------------------
    let count: i64 = conn.query_row("SELECT count(*) FROM books", [], |row| row.get(0))?;
    println!("number of books: {count}");

    // --- b) A row that may not exist: `.optional()` turns
    //        Err(QueryReturnedNoRows) into Ok(None) --------------------------
    let find_isbn = |isbn: &str| -> rusqlite::Result<Option<String>> {
        conn.query_row("SELECT title FROM books WHERE isbn = ?1", [isbn], |row| {
            row.get(0)
        })
        .optional()
    };
    println!(
        "ISBN 978-0-00-000005-9 -> {:?}",
        find_isbn("978-0-00-000005-9")?
    );
    println!(
        "ISBN 999-9-99-999999-9 -> {:?}",
        find_isbn("999-9-99-999999-9")?
    );

    // --- c) Many rows mapped to structs ---------------------------------------
    let sql = format!("{} WHERE b.genre = ?1 ORDER BY b.year", Book::SELECT);
    let mut stmt = conn.prepare(&sql)?;
    let novels: Vec<Book> = stmt
        .query_map([Genre::Novel], Book::from_row)?
        .collect::<rusqlite::Result<_>>()?;
    println!("\nnovels:");
    for b in &novels {
        println!(
            "  #{:<2} {:<34} {:<15} {} {:>9}  {} copies  tags {:?}",
            b.id,
            b.title,
            b.author,
            b.year,
            eur(b.price_cents),
            b.copies,
            b.tags
        );
    }
    debug_assert!(novels.iter().all(|b| b.genre == Genre::Novel));

    // --- d) NULL values become Option ---------------------------------------
    let mut stmt = conn.prepare("SELECT name, born, country FROM authors ORDER BY born")?;
    let authors = stmt.query_map([], |row| {
        Ok(Author {
            name: row.get(0)?,
            born: row.get(1)?,
            country: row.get(2)?,
        })
    })?;
    println!("\nauthors (NULL sorts first):");
    for author in authors {
        let a = author?;
        let born = a.born.map_or("unknown".to_string(), |y| y.to_string());
        let country = a.country.as_deref().unwrap_or("-");
        println!("  {:<16} born {:<8} {}", a.name, born, country);
    }

    // --- e) Iterating over rows by hand -------------------------------------
    let mut stmt = conn
        .prepare("SELECT genre, count(*), sum(copies) FROM books GROUP BY genre ORDER BY genre")?;
    let mut rows = stmt.query([])?;
    println!("\ncopies per genre:");
    while let Some(row) = rows.next()? {
        let genre: Genre = row.get(0)?;
        let titles: i64 = row.get(1)?;
        let copies: i64 = row.get(2)?;
        println!("  {:<9} {titles} titles, {copies} copies", genre.as_str());
    }

    // --- f) A variable number of parameters (an IN list) --------------------
    let wanted = ["Emma", "Poems", "Frankenstein", "Ulysses"];
    let placeholders = vec!["?"; wanted.len()].join(", ");
    let sql =
        format!("SELECT title, year FROM books WHERE title IN ({placeholders}) ORDER BY year");
    let mut stmt = conn.prepare(&sql)?;
    let found: Vec<(String, i64)> = stmt
        .query_map(params_from_iter(wanted), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    println!("\nlooked for {wanted:?}\nfound {found:?}");

    // --- g) LIKE with user input: escape % and _ --------------------------------
    let user_input = "the ";
    let pattern = format!(
        "{}%",
        user_input
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut stmt =
        conn.prepare("SELECT title FROM books WHERE title LIKE ?1 ESCAPE '\\' ORDER BY title")?;
    let titles: Vec<String> = stmt
        .query_map([&pattern], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    println!("\ntitles starting with '{user_input}' (LIKE is case-insensitive for ASCII):");
    for t in titles {
        println!("  {t}");
    }
    Ok(())
}

// ===========================================================================
// 5. UPDATE, DELETE and UPSERT
// ===========================================================================

fn updates_and_upserts(conn: &Connection) -> Result<()> {
    heading(5, "UPDATE, DELETE and UPSERT");

    // execute() returns the number of changed rows.
    let changed = conn.execute(
        "UPDATE books SET price_cents = price_cents * 110 / 100 WHERE genre = ?1",
        [Genre::Science],
    )?;
    println!("raised the price of {changed} science books by 10 %");

    let changed = conn.execute(
        "UPDATE books SET copies = 5 WHERE isbn = ?1",
        ["no-such-isbn"],
    )?;
    println!("update of a missing ISBN changed {changed} rows (no error!)");

    // UPSERT: a delivery of books. A new ISBN is inserted, a known ISBN only
    // gets more copies. `excluded` is the row that could not be inserted.
    let mut stmt = conn.prepare(
        "INSERT INTO books (author_id, title, isbn, genre, year, price_cents, copies)
         VALUES ((SELECT id FROM authors WHERE name = ?1), ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (isbn) DO UPDATE SET copies = copies + excluded.copies
         RETURNING id, title, copies",
    )?;
    let deliveries = [
        (
            "Jane Austen",
            "Pride and Prejudice",
            "978-0-00-000001-1",
            Genre::Novel,
            1813,
            899,
            2,
        ),
        (
            "Jules Verne",
            "Journey to the Center of the Earth",
            "978-0-00-000012-7",
            Genre::Novel,
            1864,
            799,
            1,
        ),
    ];
    for (author, title, isbn, genre, year, price, copies) in deliveries {
        let (id, title, total): (i64, String, i64) = stmt.query_row(
            params![author, title, isbn, genre, year, price, copies],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        println!("delivery of {copies}: #{id} '{title}' now has {total} copies");
    }

    // DELETE with ON DELETE CASCADE: the member's loans go with them.
    conn.execute(
        "INSERT INTO members (name, email) VALUES ('Eve Temp', 'eve@example.org')",
        [],
    )?;
    let eve = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO loans (book_id, member_id, loaned_on, due_on)
         VALUES (?1, ?2, date('now'), date('now', '+14 days'))",
        params![book_id(conn, "Poems")?, eve],
    )?;
    let loans_before = count(conn, "SELECT count(*) FROM loans")?;
    let deleted = conn.execute("DELETE FROM members WHERE id = ?1", [eve])?;
    let loans_after = count(conn, "SELECT count(*) FROM loans")?;
    println!("deleted {deleted} member; loans {loans_before} -> {loans_after} (cascade)");
    Ok(())
}

// ===========================================================================
// 6. Transactions
// ===========================================================================

/// A typed error for the business logic, so the caller can react to each case.
#[derive(Debug)]
enum LoanError {
    BookNotFound(i64),
    MemberNotFound(i64),
    NoCopyAvailable { title: String },
    LimitReached { member: String, open: i64 },
    NotOnLoan(i64),
    Db(rusqlite::Error),
}

impl fmt::Display for LoanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoanError::BookNotFound(id) => write!(f, "book #{id} does not exist"),
            LoanError::MemberNotFound(id) => write!(f, "member #{id} does not exist"),
            LoanError::NoCopyAvailable { title } => {
                write!(f, "all copies of '{title}' are on loan")
            }
            LoanError::LimitReached { member, open } => {
                write!(
                    f,
                    "{member} already has {open} books (limit {MAX_OPEN_LOANS})"
                )
            }
            LoanError::NotOnLoan(id) => write!(f, "loan #{id} is not open"),
            LoanError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for LoanError {}

impl From<rusqlite::Error> for LoanError {
    fn from(e: rusqlite::Error) -> Self {
        LoanError::Db(e)
    }
}

/// Lends a book. All checks and the INSERT run in one transaction, so two
/// librarians cannot lend the last copy at the same moment.
fn lend_book(
    conn: &mut Connection,
    book_id: i64,
    member_id: i64,
    days: i64,
) -> Result<i64, LoanError> {
    // IMMEDIATE takes the write lock at BEGIN. A DEFERRED transaction (the
    // default) would read first and could fail with SQLITE_BUSY when it later
    // tries to upgrade to a write lock.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let (title, copies): (String, i64) = tx
        .query_row(
            "SELECT title, copies FROM books WHERE id = ?1",
            [book_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(LoanError::BookNotFound(book_id))?;
    let on_loan: i64 = tx.query_row(
        "SELECT count(*) FROM loans WHERE book_id = ?1 AND returned_on IS NULL",
        [book_id],
        |row| row.get(0),
    )?;
    if on_loan >= copies {
        // Returning drops `tx` without commit -> automatic ROLLBACK.
        return Err(LoanError::NoCopyAvailable { title });
    }

    let (member, open): (String, i64) = tx
        .query_row(
            "SELECT name, (SELECT count(*) FROM loans
                           WHERE member_id = members.id AND returned_on IS NULL)
             FROM members WHERE id = ?1",
            [member_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(LoanError::MemberNotFound(member_id))?;
    if open >= MAX_OPEN_LOANS {
        return Err(LoanError::LimitReached { member, open });
    }

    let loan_id: i64 = tx.query_row(
        "INSERT INTO loans (book_id, member_id, loaned_on, due_on)
         VALUES (?1, ?2, date('now'), date('now', ?3)) RETURNING id",
        params![book_id, member_id, format!("+{days} days")],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(loan_id)
}

fn return_book(conn: &Connection, loan_id: i64) -> Result<(), LoanError> {
    let changed = conn.execute(
        "UPDATE loans SET returned_on = date('now') WHERE id = ?1 AND returned_on IS NULL",
        [loan_id],
    )?;
    if changed == 0 {
        return Err(LoanError::NotOnLoan(loan_id));
    }
    Ok(())
}

fn transactions(conn: &mut Connection) -> Result<()> {
    heading(6, "Transactions");

    let emma = book_id(conn, "Emma")?;
    let david = member_id(conn, "david@example.org")?;
    let bob = member_id(conn, "bob@example.org")?;
    let alice = member_id(conn, "alice@example.org")?;

    let loan = lend_book(conn, emma, david, 14)?;
    println!("lent 'Emma' to David: loan #{loan}");

    match lend_book(conn, emma, bob, 14) {
        Ok(id) => println!("unexpected: loan #{id}"),
        Err(e) => println!("lending 'Emma' to Bob failed: {e}"),
    }

    // Alice has 2 open loans; the 3rd works, the 4th hits the limit.
    for title in ["Poems", "Through the Looking-Glass"] {
        match lend_book(conn, book_id(conn, title)?, alice, 21) {
            Ok(id) => println!("lent '{title}' to Alice: loan #{id}"),
            Err(e) => println!("lending '{title}' to Alice failed: {e}"),
        }
    }
    match lend_book(conn, 999, alice, 7) {
        Ok(id) => println!("unexpected: loan #{id}"),
        Err(e) => println!("lending book #999 failed: {e}"),
    }

    return_book(conn, loan)?;
    println!("David returned loan #{loan}");
    if let Err(e) = return_book(conn, loan) {
        println!("returning it a second time failed: {e}");
    }

    // --- Rollback: nothing of a failed transaction remains ------------------
    let before = count(conn, "SELECT count(*) FROM members")?;
    {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO members (name, email) VALUES ('Temp 1', 't1@example.org')",
            [],
        )?;
        tx.execute(
            "INSERT INTO members (name, email) VALUES ('Temp 2', 't2@example.org')",
            [],
        )?;
        tx.rollback()?; // or simply drop `tx`
    }
    let after = count(conn, "SELECT count(*) FROM members")?;
    println!("\nrollback: members {before} -> {after}");

    // --- Savepoints: undo part of a transaction ------------------------------
    let mut tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO members (name, email) VALUES ('Frank Moss', 'frank@example.org')",
        [],
    )?;
    {
        let mut sp = tx.savepoint()?;
        // This fails (the e-mail exists, compared case-insensitively) ...
        let result = sp.execute(
            "INSERT INTO members (name, email) VALUES ('Alice Again', 'ALICE@example.org')",
            [],
        );
        println!("insert inside savepoint: {}", describe(&result));
        // ... so only the work since the savepoint is undone.
        sp.rollback()?;
    }
    tx.commit()?;
    let frank = conn
        .query_row(
            "SELECT name FROM members WHERE email = 'frank@example.org'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    println!(
        "after savepoint rollback + commit, Frank exists: {}",
        frank.is_some()
    );
    Ok(())
}

// ===========================================================================
// 7. Constraint errors
// ===========================================================================

fn constraint_errors(conn: &Connection) -> Result<()> {
    heading(7, "Constraint errors");

    let tries: [(&str, &str); 6] = [
        (
            "duplicate ISBN",
            "INSERT INTO books (author_id, title, isbn, genre, year, price_cents)
                            VALUES (1, 'Copy', '978-0-00-000001-1', 'novel', 1813, 100)",
        ),
        (
            "unknown author",
            "INSERT INTO books (author_id, title, isbn, genre, year, price_cents)
                            VALUES (42, 'Ghost', 'x-1', 'novel', 2000, 100)",
        ),
        (
            "delete author that has books",
            "DELETE FROM authors WHERE name = 'Jane Austen'",
        ),
        (
            "invalid genre",
            "INSERT INTO books (author_id, title, isbn, genre, year, price_cents)
                           VALUES (1, 'Cookbook', 'x-2', 'cooking', 2000, 100)",
        ),
        (
            "missing NOT NULL value",
            "INSERT INTO members (name) VALUES ('No Mail')",
        ),
        (
            "text in a STRICT INTEGER column",
            "UPDATE books SET year = 'long ago' WHERE id = 1",
        ),
    ];
    for (what, sql) in tries {
        let result = conn.execute(sql, []);
        println!("{what:<32} -> {}", describe(&result));
    }

    // Errors that rusqlite reports itself, before SQLite is involved:
    let wrong_param_count = conn.execute("UPDATE books SET copies = ?1 WHERE id = ?2", [1]);
    println!(
        "{:<32} -> {}",
        "wrong number of parameters",
        describe(&wrong_param_count)
    );
    let wrong_type: rusqlite::Result<i64> =
        conn.query_row("SELECT title FROM books WHERE id = 1", [], |row| row.get(0));
    println!("{:<32} -> {}", "TEXT read as i64", describe(&wrong_type));
    Ok(())
}

/// Turns a rusqlite result into a short description. This is how the
/// different kinds of errors are told apart.
fn describe<T: fmt::Debug>(result: &rusqlite::Result<T>) -> String {
    use rusqlite::Error;
    match result {
        Ok(value) => format!("ok: {value:?}"),
        Err(Error::SqliteFailure(err, msg)) if err.code == ErrorCode::ConstraintViolation => {
            let kind = match err.extended_code {
                ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => "UNIQUE",
                ffi::SQLITE_CONSTRAINT_FOREIGNKEY => "FOREIGN KEY",
                ffi::SQLITE_CONSTRAINT_CHECK => "CHECK",
                ffi::SQLITE_CONSTRAINT_NOTNULL => "NOT NULL",
                ffi::SQLITE_CONSTRAINT_DATATYPE => "DATATYPE",
                // ON DELETE RESTRICT is checked by an internal trigger.
                ffi::SQLITE_CONSTRAINT_TRIGGER => "FOREIGN KEY (RESTRICT)",
                _ => "other",
            };
            format!(
                "{kind} violation ({})",
                msg.as_deref().unwrap_or("no message")
            )
        }
        Err(Error::SqliteFailure(err, msg)) => {
            format!(
                "SQLite error {:?}: {}",
                err.code,
                msg.as_deref().unwrap_or("")
            )
        }
        Err(Error::InvalidColumnType(index, name, ty)) => {
            format!("column {index} '{name}' has type {ty}, which does not fit")
        }
        Err(e) => format!("{e}"),
    }
}

// ===========================================================================
// 8. Reports
// ===========================================================================

fn reports(conn: &Connection) -> Result<()> {
    heading(8, "Reports: JOIN, GROUP BY, window functions, CTE, views");

    // --- LEFT JOIN + GROUP BY: members without loans are listed too ---------
    let mut stmt = conn.prepare(
        "SELECT m.name,
                count(l.id)                                      AS total,
                count(l.id) FILTER (WHERE l.returned_on IS NULL) AS open
         FROM members m
         LEFT JOIN loans l ON l.member_id = m.id
         GROUP BY m.id
         ORDER BY total DESC, m.name",
    )?;
    println!("loans per member:");
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (name, total, open) = row?;
        println!("  {name:<12} {total} loans, {open} open");
    }

    // --- Window function: rank inside each genre without collapsing rows ---
    let mut stmt = conn.prepare(
        "SELECT genre, title, price_cents,
                rank() OVER (PARTITION BY genre ORDER BY price_cents DESC) AS place,
                round(100.0 * price_cents / sum(price_cents) OVER (PARTITION BY genre), 1)
                    AS share
         FROM books
         ORDER BY genre, place",
    )?;
    println!("\nbooks ranked by price within their genre:");
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, f64>(4)?,
        ))
    })?;
    for row in rows {
        let (genre, title, price, place, share) = row?;
        println!(
            "  {genre:<9} {place}. {:<40} {:>9} {share:>5.1} %",
            truncate(&title, 40),
            eur(price)
        );
    }

    // --- Recursive CTE: a calendar of the last 7 days, also days w/o loans --
    let mut stmt = conn.prepare(
        "WITH RECURSIVE days(day) AS (
             SELECT date('now', '-6 days')
             UNION ALL
             SELECT date(day, '+1 day') FROM days WHERE day < date('now')
         )
         SELECT days.day, count(loans.id)
         FROM days LEFT JOIN loans ON loans.loaned_on = days.day
         GROUP BY days.day ORDER BY days.day",
    )?;
    println!("\nnew loans per day (last 7 days):");
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (day, n) = row?;
        println!("  {day} {}", "#".repeat(n as usize));
    }

    // --- A view is queried like a table -----------------------------------
    let mut stmt =
        conn.prepare("SELECT title, member, due_on, days_overdue FROM open_loans ORDER BY due_on")?;
    println!("\nopen loans (view 'open_loans'):");
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (title, member, due, overdue) = row?;
        let state = if overdue > 0 {
            format!("OVERDUE by {overdue} days")
        } else {
            "ok".into()
        };
        println!(
            "  {:<34} {member:<12} due {due}  {state}",
            truncate(&title, 34)
        );
    }
    Ok(())
}

// ===========================================================================
// 9. JSON
// ===========================================================================

fn json_columns(conn: &Connection) -> Result<()> {
    heading(9, "JSON columns");

    // json_each() turns a JSON array into rows, so it can be joined.
    let mut stmt = conn.prepare(
        "SELECT tag.value, count(*) FROM books, json_each(books.tags) AS tag
         GROUP BY tag.value ORDER BY count(*) DESC, tag.value LIMIT 5",
    )?;
    let top: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    println!("most used tags: {top:?}");

    let mut stmt = conn.prepare(
        "SELECT title FROM books
         WHERE EXISTS (SELECT 1 FROM json_each(books.tags) WHERE value = ?1)
         ORDER BY title",
    )?;
    let travel: Vec<String> = stmt
        .query_map(["travel"], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    println!("books tagged 'travel': {travel:?}");

    // Modify JSON in SQL: append a tag ('$[#]' = after the last element).
    conn.execute(
        "UPDATE books SET tags = json_insert(tags, '$[#]', ?1) WHERE title = ?2",
        ["gothic", "Frankenstein"],
    )?;
    // `->>` extracts a value as SQL type, `->` as JSON text.
    let (first, all): (String, String) = conn.query_row(
        "SELECT tags ->> '$[0]', tags FROM books WHERE title = 'Frankenstein'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    println!("Frankenstein tags now {all}, first tag '{first}'");

    // Build JSON in SQL, e.g. for a web API.
    let json: String = conn.query_row(
        "SELECT json_group_array(json_object('title', title, 'year', year))
         FROM books WHERE genre = 'children'",
        [],
        |r| r.get(0),
    )?;
    println!("children's books as JSON: {json}");
    Ok(())
}

// ===========================================================================
// 10. Full-text search
// ===========================================================================

fn full_text_search(conn: &Connection) -> Result<()> {
    heading(10, "Full-text search (FTS5)");

    let mut stmt = conn.prepare(
        "SELECT b.title,
                highlight(books_fts, 1, '[', ']') AS summary,
                bm25(books_fts)                   AS score
         FROM books_fts
         JOIN books b ON b.id = books_fts.rowid
         WHERE books_fts MATCH ?1
         ORDER BY score            -- bm25: smaller = better match
         LIMIT 3",
    )?;
    for query in [
        "travel",
        "monster OR creature",
        "\"natural selection\"",
        "title:alice",
        "roma*",
    ] {
        println!("search {query}:");
        let rows = stmt.query_map([query], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })?;
        for row in rows {
            let (title, summary, score) = row?;
            println!(
                "  {score:6.2}  {title}\n          {}",
                truncate(&summary, 90)
            );
        }
    }

    // The triggers keep the index in sync with UPDATEs of the books table.
    conn.execute(
        "UPDATE books SET summary = summary || ' Also a story about a balloon.'
         WHERE title = 'Around the World in Eighty Days'",
        [],
    )?;
    let hits: i64 = conn.query_row(
        "SELECT count(*) FROM books_fts WHERE books_fts MATCH 'balloon'",
        [],
        |r| r.get(0),
    )?;
    println!("after an UPDATE, 'balloon' finds {hits} book");

    // A syntax error in the search string is a normal SQLite error.
    let bad: rusqlite::Result<i64> = conn.query_row(
        "SELECT count(*) FROM books_fts WHERE books_fts MATCH ?1",
        ["AND OR"],
        |r| r.get(0),
    );
    println!("invalid search 'AND OR' -> {}", describe(&bad));
    Ok(())
}

// ===========================================================================
// 11. BLOBs
// ===========================================================================

fn blobs(conn: &Connection) -> Result<()> {
    heading(11, "BLOBs");

    // A fake "image": the PNG signature followed by some bytes.
    let mut image: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    image.extend((0..4_000u32).map(|i| (i * 7 % 256) as u8));

    // Vec<u8> and &[u8] are stored as BLOB.
    let book = book_id(conn, "Frankenstein")?;
    conn.execute(
        "INSERT INTO covers (book_id, mime, data) VALUES (?1, ?2, ?3)",
        params![book, "image/png", image],
    )?;

    let (mime, size, data): (String, i64, Vec<u8>) = conn.query_row(
        "SELECT mime, length(data), data FROM covers WHERE book_id = ?1",
        [book],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    println!(
        "stored and read back a {mime} cover of {size} bytes, identical: {}, starts with {:02X?}",
        data == image,
        &data[..4]
    );
    // For files of many MB, look at the "blob" feature (incremental BLOB I/O)
    // or store the file on disk and only its path in the database.
    Ok(())
}

// ===========================================================================
// 12. User-defined functions
// ===========================================================================

fn user_defined_functions(conn: &Connection) -> Result<()> {
    heading(12, "SQL functions written in Rust");

    // DETERMINISTIC (same input -> same output) allows SQLite to use the
    // function in indexes and to optimise calls.
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;

    conn.create_scalar_function("initials", 1, flags, |ctx| {
        let name: String = ctx.get(0)?;
        Ok(name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .map(|c| format!("{c}."))
            .collect::<String>())
    })?;

    conn.create_scalar_function("eur", 1, flags, |ctx| {
        let cents: Option<i64> = ctx.get(0)?; // NULL in -> NULL out
        Ok(cents.map(eur))
    })?;

    let mut stmt = conn.prepare(
        "SELECT initials(a.name), b.title, eur(b.price_cents)
         FROM books b JOIN authors a ON a.id = b.author_id
         WHERE b.price_cents > 1000 ORDER BY b.price_cents DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (initials, title, price) = row?;
        println!("  {initials:<6} {:<45} {price:>9}", truncate(&title, 45));
    }
    // Note: the functions exist only on this connection. Every connection
    // must register them again.
    Ok(())
}

// ===========================================================================
// 13. Bulk inserts
// ===========================================================================

fn bulk_inserts(conn: &mut Connection) -> Result<()> {
    heading(13, "Bulk inserts and the cost of a commit");

    conn.execute_batch(
        "CREATE TABLE numbers (n INTEGER PRIMARY KEY, square INTEGER NOT NULL) STRICT",
    )?;

    // Without an explicit transaction, every statement is its own
    // transaction with its own commit (and journal write).
    let n_single = 2_000;
    let start = Instant::now();
    {
        let mut stmt = conn.prepare("INSERT INTO numbers (n, square) VALUES (?1, ?2)")?;
        for n in 0..n_single {
            stmt.execute([n, n * n])?;
        }
    }
    let t_single = start.elapsed();

    // The same work inside one transaction: one commit at the end.
    let n_tx = 200_000;
    let start = Instant::now();
    {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT INTO numbers (n, square) VALUES (?1, ?2)")?;
            for n in n_single..n_single + n_tx {
                stmt.execute([n, n * n])?;
            }
        }
        tx.commit()?;
    }
    let t_tx = start.elapsed();

    let rate = |rows: i64, t: Duration| rows as f64 / t.as_secs_f64();
    println!(
        "{n_single:>7} rows, one commit per row: {:>8.1} ms  ({:>10.0} rows/s)",
        t_single.as_secs_f64() * 1e3,
        rate(n_single, t_single)
    );
    println!(
        "{n_tx:>7} rows, one transaction:    {:>8.1} ms  ({:>10.0} rows/s)",
        t_tx.as_secs_f64() * 1e3,
        rate(n_tx, t_tx)
    );

    let sum: i64 = conn.query_row("SELECT sum(square) % 1000003 FROM numbers", [], |r| {
        r.get(0)
    })?;
    println!("checksum over {} rows: {sum}", n_single + n_tx);
    conn.execute_batch("DROP TABLE numbers")?;
    Ok(())
}

// ===========================================================================
// 14. Several threads
// ===========================================================================

fn multiple_threads(conn: &Connection) -> Result<()> {
    heading(14, "Several threads, one database");

    // `Connection` is Send but not Sync: it can be moved to another thread,
    // but not shared. The usual pattern is one connection per thread (or a
    // pool such as r2d2_sqlite / deadpool-sqlite).
    // SQLite allows many readers but only ONE writer at a time; busy_timeout
    // makes the other writers wait instead of failing.
    let books: Vec<i64> = conn
        .prepare("SELECT id FROM books")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    let workers = 4;
    let per_worker = 250;
    let start = Instant::now();
    let handles: Vec<_> = (0..workers)
        .map(|worker| {
            let books = books.clone();
            thread::spawn(move || -> Result<()> {
                let mut conn = Connection::open(DB_PATH)?;
                configure(&conn)?;
                // Write in batches of 25 rows; each batch is one transaction.
                for batch in 0..per_worker / 25 {
                    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                    {
                        let mut stmt = tx.prepare_cached(
                            "INSERT INTO page_views (book_id, worker) VALUES (?1, ?2)",
                        )?;
                        for i in 0..25 {
                            let book = books[(batch * 25 + i + worker) % books.len()];
                            stmt.execute(params![book, worker as i64])?;
                        }
                    }
                    tx.commit()?;
                }
                Ok(())
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("worker thread panicked")?;
    }

    let mut stmt = conn.prepare("SELECT worker, count(*) FROM page_views GROUP BY worker")?;
    let per: Vec<(i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let total: i64 = per.iter().map(|(_, n)| n).sum();
    println!(
        "{workers} threads wrote {total} rows in {:.0} ms, rows per worker: {per:?}",
        start.elapsed().as_secs_f64() * 1e3
    );
    if total != (workers * per_worker) as i64 {
        bail!("expected {} rows, found {total}", workers * per_worker);
    }
    Ok(())
}

// ===========================================================================
// 15. Query plans
// ===========================================================================

fn query_plans(conn: &Connection) -> Result<()> {
    heading(15, "Indexes and EXPLAIN QUERY PLAN");

    let query = "SELECT title FROM books WHERE year BETWEEN 1850 AND 1870";
    print_plan(conn, query)?;
    conn.execute_batch("CREATE INDEX idx_books_year ON books(year)")?;
    println!("-- after CREATE INDEX idx_books_year:");
    print_plan(conn, query)?;
    print_plan(
        conn,
        "SELECT count(*) FROM loans WHERE book_id = 3 AND returned_on IS NULL",
    )?;

    // ANALYZE collects statistics that help the query planner choose indexes.
    // `PRAGMA optimize` does this cheaply; run it before closing long-lived
    // connections.
    conn.execute_batch("PRAGMA optimize")?;
    Ok(())
}

fn print_plan(conn: &Connection, sql: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    // Columns: id, parent, notused, detail
    let details: Vec<String> = stmt
        .query_map([], |r| r.get(3))?
        .collect::<rusqlite::Result<_>>()?;
    println!("{sql}\n    plan: {}", details.join(" / "));
    Ok(())
}

// ===========================================================================
// 16. Backup and in-memory databases
// ===========================================================================

fn backup_and_memory(conn: &Connection) -> Result<()> {
    heading(16, "Backup, in-memory database, VACUUM INTO");

    // The online backup API copies a database that is in use, page by page.
    conn.backup(
        "main",
        BACKUP_PATH,
        Some(|p: Progress| {
            if p.remaining == 0 {
                println!("backup: copied {} pages to '{BACKUP_PATH}'", p.pagecount);
            }
        }),
    )?;

    // Load the backup into RAM. In-memory databases are ideal for tests:
    // fast, isolated and gone when the connection is dropped.
    let mut mem = Connection::open_in_memory()?;
    mem.restore("main", BACKUP_PATH, None::<fn(Progress)>)?;
    let books = count(&mem, "SELECT count(*) FROM books")?;
    let views = count(&mem, "SELECT count(*) FROM page_views")?;
    println!("in-memory copy: {books} books, {views} page views");

    // Changes to the copy do not touch the original.
    mem.execute("DELETE FROM page_views", [])?;
    println!(
        "after DELETE in the copy: copy {} page views, original {} page views",
        count(&mem, "SELECT count(*) FROM page_views")?,
        count(conn, "SELECT count(*) FROM page_views")?
    );

    // VACUUM INTO writes a compacted copy with a single SQL statement.
    conn.execute("VACUUM INTO ?1", [VACUUM_PATH])?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    println!("VACUUM INTO '{VACUUM_PATH}' done, integrity_check: {integrity}");
    for path in [DB_PATH, BACKUP_PATH, VACUUM_PATH] {
        println!("  {path:<20} {:>7} bytes", std::fs::metadata(path)?.len());
    }
    println!("  ('{DB_PATH}' is small because recent changes are still in '{DB_PATH}-wal';");
    println!("   the backup is large because it copies the free pages of the dropped table)");
    Ok(())
}

// ===========================================================================
// Helpers
// ===========================================================================

fn heading(number: u32, title: &str) {
    println!(
        "\n=== {number}. {title} {}",
        "=".repeat(60usize.saturating_sub(title.len()))
    );
}

fn count(conn: &Connection, sql: &str) -> rusqlite::Result<i64> {
    conn.query_row(sql, [], |row| row.get(0))
}

/// Money is stored as integer cents: floating point cannot represent 0.10 exactly.
fn eur(cents: i64) -> String {
    format!("{}.{:02} EUR", cents / 100, cents % 100)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let short: String = s.chars().take(max - 3).collect();
        format!("{short}...")
    }
}

/// Removes a database file together with its WAL and shared-memory files.
fn remove_db_files(path: &str) -> Result<()> {
    for file in [
        path.to_string(),
        format!("{path}-wal"),
        format!("{path}-shm"),
    ] {
        if Path::new(&file).exists() {
            std::fs::remove_file(&file).with_context(|| format!("cannot remove {file}"))?;
        }
    }
    Ok(())
}
