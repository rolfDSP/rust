# Code Documentation

This document explains how the example in `src/main.rs` works and why it is written the way it is. It assumes you know some Rust and a little SQL. For a quick start, see [README.md](README.md).

## Contents

1. [SQLite in a nutshell](#1-sqlite-in-a-nutshell)
2. [Project setup: `Cargo.toml`](#2-project-setup-cargotoml)
3. [The rusqlite API in one page](#3-the-rusqlite-api-in-one-page)
4. [Opening a database and PRAGMAs](#4-opening-a-database-and-pragmas)
5. [Schema and migrations](#5-schema-and-migrations)
6. [Inserting data](#6-inserting-data)
7. [Queries and type mapping](#7-queries-and-type-mapping)
8. [UPDATE, DELETE and UPSERT](#8-update-delete-and-upsert)
9. [Transactions](#9-transactions)
10. [Error handling](#10-error-handling)
11. [Reports: joins, window functions, CTEs, views, dates](#11-reports-joins-window-functions-ctes-views-dates)
12. [JSON](#12-json)
13. [Full-text search with FTS5](#13-full-text-search-with-fts5)
14. [BLOBs](#14-blobs)
15. [User-defined functions](#15-user-defined-functions)
16. [Performance: bulk inserts](#16-performance-bulk-inserts)
17. [Threads and concurrency](#17-threads-and-concurrency)
18. [Indexes and query plans](#18-indexes-and-query-plans)
19. [Backup and in-memory databases](#19-backup-and-in-memory-databases)
20. [Pitfalls](#20-pitfalls)
21. [Ideas for experiments](#21-ideas-for-experiments)
22. [Further reading](#22-further-reading)

---

## 1. SQLite in a nutshell

SQLite is not a server. It is a C library that reads and writes a single database file directly. There is no installation, no user management and no network: the program *is* the database engine.

```text
 Client/server (PostgreSQL, MySQL)          Embedded (SQLite)
 ┌─────────┐   TCP   ┌──────────┐           ┌──────────────────────┐
 │ program │ ──────► │  server  │──► files  │ program + SQLite lib │──► library.db
 └─────────┘         └──────────┘           └──────────────────────┘
```

Good fits: desktop and mobile apps, command-line tools, tests, caches, websites with moderate write load, data files for exchange. Less suited: many machines writing to the same database over a network, or very high write concurrency (SQLite allows one writer at a time).

Things that differ from other SQL databases:

- **Flexible types.** A normal column accepts any type of value; `INTEGER` is just a preference. `STRICT` tables (used in this example) enforce the declared type. The storage classes are `NULL`, `INTEGER` (64-bit), `REAL` (64-bit float), `TEXT` and `BLOB`.
- **No date type.** Dates are stored as ISO 8601 text (`'2026-11-11'`), Unix time integers or Julian day numbers, and handled by the date functions.
- **No boolean type.** `true`/`false` are the integers 1 and 0.
- **Foreign keys are off by default** and must be enabled per connection.
- **`INTEGER PRIMARY KEY`** is an alias for the internal `rowid` and is assigned automatically.

## 2. Project setup: `Cargo.toml`

```toml
rusqlite = { version = "0.42", features = ["bundled", "functions", "backup"] }
```

- **`bundled`** compiles the SQLite C source that ships with `libsqlite3-sys` into the program. You get a recent SQLite (3.54 here) with FTS5 and JSON, independent of the version installed on the system. Without it, rusqlite links the system's `libsqlite3`.
- **`functions`** enables `create_scalar_function` and friends (section 15).
- **`backup`** enables the online backup API (section 19).

Other useful features: `chrono`, `time`, `jiff`, `uuid`, `serde_json`, `url` (implement `ToSql`/`FromSql` for these types), `blob` (incremental BLOB I/O), `hooks` (update/commit hooks), `bundled-sqlcipher` (encrypted databases), `array`, `vtab`, `csvtab`.

`anyhow` is used for simple error propagation in `main`, `serde_json` to convert the tag lists.

## 3. The rusqlite API in one page

| Type / method | Purpose |
|---------------|---------|
| `Connection` | one open database. `execute`, `execute_batch`, `query_row`, `prepare`, `transaction` |
| `conn.execute(sql, params)` | runs one statement, returns the number of changed rows |
| `conn.execute_batch(sql)` | runs several `;`-separated statements without parameters |
| `conn.query_row(sql, params, \|row\| ...)` | runs a query and maps the **first** row; error if there is none |
| `conn.prepare(sql)` → `Statement` | compiles a statement for repeated use |
| `conn.prepare_cached(sql)` | same, but keeps it in a per-connection LRU cache |
| `stmt.query_map(params, f)` | iterator of `Result<T>`, one per row |
| `stmt.query(params)` → `Rows` | manual iteration with `rows.next()?` |
| `row.get::<_, T>(idx_or_name)` | reads a column as Rust type `T` |
| `conn.transaction()` → `Transaction` | `BEGIN`; `commit()` or `rollback()`; **drop = rollback** |
| `.optional()` (`OptionalExtension`) | turns "no rows" into `Ok(None)` |

Parameters can be given in several forms:

```rust
conn.execute("... VALUES (?1, ?2)", params![name, born])?;      // macro, mixed types
conn.execute("... VALUES (?1, ?2)", (name, email))?;            // tuple
conn.execute("... WHERE id = ?1", [id])?;                       // array, same type
conn.execute("... VALUES (:title)", named_params! { ":title": t })?;
stmt.query_map(params_from_iter(vec), ...)?;                    // any iterator
conn.execute("DELETE FROM t", [])?;                             // no parameters
```

**Always pass values as parameters**, never with `format!` into the SQL. Parameters cannot cause SQL injection, need no quoting and allow SQLite to reuse the compiled statement. `format!` is used in this example only for things that cannot be parameters: the number of `?` in an `IN` list and table or column names fixed in the code.

## 4. Opening a database and PRAGMAs

`open_database` and `configure`:

```rust
let conn = Connection::open("library.db")?;   // creates the file if needed
conn.execute_batch(
    "PRAGMA journal_mode = WAL;
     PRAGMA synchronous = NORMAL;
     PRAGMA foreign_keys = ON;")?;
conn.busy_timeout(Duration::from_secs(5))?;
```

| Setting | Why |
|---------|-----|
| `journal_mode = WAL` | Write-ahead logging: changes are appended to `library.db-wal` and copied into the main file later ("checkpoint"). Readers never block the writer and the writer never blocks readers. Stored in the file, so it only has to be set once. |
| `synchronous = NORMAL` | In WAL mode this is crash-safe for the database (a power cut can lose only the last commits, never corrupt the file) and much faster than `FULL`. |
| `foreign_keys = ON` | Without it, `REFERENCES` clauses are ignored. Must be set for **every** connection. |
| `busy_timeout` | When another connection writes, wait (up to 5 s) instead of failing at once with `database is locked`. |

`configure` is a separate function because the worker threads in section 17 open their own connections, which need the same settings.

## 5. Schema and migrations

A program that lives longer than one version must change its schema over time. The example keeps an ordered list of migrations, `MIGRATIONS`, and stores the current version in `PRAGMA user_version`, a free integer in the database header:

```rust
let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
    let tx = conn.transaction()?;
    tx.execute_batch(sql)?;
    tx.pragma_update(None, "user_version", index as i64 + 1)?;
    tx.commit()?;
}
```

Because DDL (`CREATE`, `ALTER`, `DROP`) is transactional in SQLite, a migration that fails half-way leaves nothing behind. Calling `migrate` a second time prints "schema is up to date". Rule: never edit a migration that has been released, always append a new one. For larger projects the crate [`rusqlite_migration`](https://crates.io/crates/rusqlite_migration) implements the same idea.

The schema shows the most important constraints:

| Constraint | Example | Effect |
|------------|---------|--------|
| `PRIMARY KEY` | `id INTEGER PRIMARY KEY` | unique row id, assigned automatically |
| `NOT NULL` | `title TEXT NOT NULL` | value is required |
| `UNIQUE` | `isbn TEXT NOT NULL UNIQUE` | no duplicates (creates an index) |
| `COLLATE NOCASE` | `email ... UNIQUE COLLATE NOCASE` | `ALICE@...` and `alice@...` count as equal |
| `CHECK` | `CHECK (copies >= 0)`, `CHECK (json_valid(tags))` | any boolean expression |
| `DEFAULT` | `DEFAULT (date('now'))` | value when the column is omitted |
| `REFERENCES ... ON DELETE CASCADE` | loans → members | deleting a member deletes their loans |
| `REFERENCES ... ON DELETE RESTRICT` | books → authors | an author with books cannot be deleted |
| `STRICT` | `) STRICT;` | values must match the declared column type |

Migration 2 adds a table for cover images, the FTS5 index with its triggers (section 13) and the view `open_loans`. Migration 3 shows `ALTER TABLE ... ADD COLUMN` on a table that may already contain data, and adds `page_views`.

## 6. Inserting data

`seed_data` runs all inserts in **one transaction**, which is both faster (section 16) and all-or-nothing. It shows four styles:

**a) Prepared statement, positional parameters, `RETURNING`.** `RETURNING id` makes the `INSERT` return the new id like a `SELECT`, so it is run with `query_row`:

```rust
let mut stmt = tx.prepare("INSERT INTO authors (name, born, country) VALUES (?1, ?2, ?3) RETURNING id")?;
let id: i64 = stmt.query_row(params![name, born, country], |row| row.get(0))?;
```

`born` is an `Option<i64>`: `None` is stored as `NULL`.

**b) Named parameters** (`:title`) are easier to read when a statement has many of them. The author id is looked up by a subquery inside the `INSERT`. `Genre` is passed directly because it implements `ToSql` (section 7).

**c) `execute` + `last_insert_rowid()`**: the classic way to get the id of the row just inserted on this connection.

**d) Dates computed by SQLite**: `date('now', '-20 days')`. A `NULL` modifier gives a `NULL` date, which is used for "not returned yet".

The statements live in their own `{ }` block because a `Statement` borrows the transaction: it must be dropped before `tx.commit()` can take ownership of `tx`.

## 7. Queries and type mapping

### Rust ↔ SQLite types

| Rust | SQLite |
|------|--------|
| `i8`…`i64`, `u8`…`u32`, `bool` | INTEGER |
| `f32`, `f64` | REAL |
| `String`, `&str` | TEXT |
| `Vec<u8>`, `&[u8]` | BLOB |
| `Option<T>` | `T` or NULL |
| `rusqlite::types::Value` | any (dynamic) |

`u64`/`usize` need the `fallible_uint` feature, because they may not fit in SQLite's signed 64-bit integer. Reading a `NULL` into a non-`Option` type, or `TEXT` into `i64`, gives `Error::InvalidColumnType`.

### Custom types: `ToSql` and `FromSql`

`Genre` is an enum stored as text. Two small trait implementations make it a first-class SQL type:

```rust
impl ToSql for Genre {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> { Ok(ToSqlOutput::from(self.as_str())) }
}
impl FromSql for Genre {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? { "novel" => Ok(Genre::Novel), /* ... */
            other => Err(FromSqlError::Other(format!("unknown genre '{other}'").into())) }
    }
}
```

The `CHECK (genre IN (...))` in the schema guarantees that the database never contains a value the Rust code cannot read.

### The query patterns in `queries`

- **a) One value:** `query_row("SELECT count(*) ...", [], |row| row.get(0))`.
- **b) Maybe one row:** `query_row(...).optional()?` returns `Option<T>`. Without `.optional()`, a missing row is the error `QueryReturnedNoRows`.
- **c) Rows to structs:** `Book::from_row` is a plain function `fn(&Row) -> Result<Book>` and is passed directly to `query_map`. `collect::<rusqlite::Result<Vec<_>>>()` stops at the first error. `Book::SELECT` keeps the column list in one place.
- **d) NULL:** `born: Option<i64>`; in SQLite `NULL` sorts before all other values.
- **e) Manual iteration:** `let mut rows = stmt.query([])?; while let Some(row) = rows.next()? { ... }` — useful when you want to stop early or process rows without collecting them.
- **f) Dynamic `IN` list:** SQL has no array parameter, so the right number of `?` is generated and the values are bound with `params_from_iter`.
- **g) `LIKE` with user input:** `%` and `_` in the input are escaped and `ESCAPE '\'` tells SQLite about it. Otherwise a user searching for `100%` would match everything that starts with `100`.

## 8. UPDATE, DELETE and UPSERT

`execute` returns the number of changed rows. An `UPDATE` that matches nothing is **not** an error (`0 rows`); check the count when it matters, as `return_book` does.

**UPSERT** inserts a row, or updates the existing one when a `UNIQUE` constraint conflicts:

```sql
INSERT INTO books (...) VALUES (...)
ON CONFLICT (isbn) DO UPDATE SET copies = copies + excluded.copies
RETURNING id, title, copies
```

`excluded` is the row that could not be inserted. A delivery of a known book increases `copies`, a new ISBN creates a new book.

`DELETE FROM members` shows `ON DELETE CASCADE`: the member's loans are deleted with them. This only works because `foreign_keys = ON`.

## 9. Transactions

Every statement outside an explicit transaction runs in its own transaction ("autocommit"). An explicit transaction groups statements so that they succeed or fail together and see a consistent state.

```rust
let tx = conn.transaction()?;   // BEGIN
tx.execute(...)?;               // on error: `?` returns, tx is dropped -> ROLLBACK
tx.commit()?;                   // COMMIT
```

The key Rust feature: **dropping a `Transaction` without `commit()` rolls it back.** Every early `return` and every `?` therefore undoes the transaction automatically.

### Business rules: `lend_book`

`lend_book` checks that the book exists, that a copy is free and that the member has fewer than `MAX_OPEN_LOANS` books, then inserts the loan. Checks and insert must be in one transaction; otherwise two librarians could both see "1 copy free" and both lend it.

It uses `TransactionBehavior::Immediate` (`BEGIN IMMEDIATE`), which takes the write lock right away. The default `DEFERRED` transaction starts as a reader and upgrades to a writer at the first write; if another connection wrote in between, the upgrade fails with `SQLITE_BUSY` and `busy_timeout` cannot help. **Rule: use `Immediate` for every transaction that reads and then writes.**

The function returns its own error type `LoanError`, so the caller can distinguish "no copy available" from a database error. `impl From<rusqlite::Error> for LoanError` makes `?` work on rusqlite calls.

### Savepoints

A savepoint is a transaction inside a transaction. In the example, a member is inserted, then a savepoint is opened, a failing insert is tried and only the savepoint is rolled back. The first insert survives the final `commit`.

## 10. Error handling

`rusqlite::Error` is an enum. The most important variants:

| Variant | Meaning |
|---------|---------|
| `SqliteFailure(ffi::Error, Option<String>)` | an error reported by SQLite; `.code` is an `ErrorCode`, `.extended_code` the exact reason |
| `QueryReturnedNoRows` | `query_row` found no row (use `.optional()`) |
| `InvalidColumnType(idx, name, type)` | the column value does not fit the Rust type |
| `InvalidParameterCount(got, expected)` | wrong number of parameters |
| `FromSqlConversionFailure(...)` | your `FromSql` conversion failed |

`describe` shows how to recognise constraint violations, e.g. to answer "this e-mail is already registered" instead of "internal error":

```rust
Err(Error::SqliteFailure(err, msg)) if err.code == ErrorCode::ConstraintViolation =>
    match err.extended_code {
        ffi::SQLITE_CONSTRAINT_UNIQUE      => ...,
        ffi::SQLITE_CONSTRAINT_FOREIGNKEY  => ...,
        ffi::SQLITE_CONSTRAINT_CHECK       => ...,
        ffi::SQLITE_CONSTRAINT_NOTNULL     => ...,
        ffi::SQLITE_CONSTRAINT_DATATYPE    => ...,   // STRICT table
        ffi::SQLITE_CONSTRAINT_TRIGGER     => ...,   // also ON DELETE RESTRICT
```

Other `ErrorCode`s you will meet: `DatabaseBusy` (lock not obtained within the timeout), `ReadOnly`, `DiskFull`, `CannotOpen`, `NotADatabase`.

## 11. Reports: joins, window functions, CTEs, views, dates

- **`LEFT JOIN` + `GROUP BY`**: lists every member, including those without loans (Frank: `0 loans`). `count(l.id)` counts only non-NULL ids. `count(...) FILTER (WHERE ...)` counts only matching rows within the group.
- **Window functions** compute values over a set of rows *without* merging them: `rank() OVER (PARTITION BY genre ORDER BY price_cents DESC)` numbers the books inside each genre, and `sum(...) OVER (PARTITION BY genre)` gives each row the total of its genre to compute a percentage. Equal prices share a rank (two books at 7.99 are both 2nd, the next one is 4th).
- **Recursive CTE** (`WITH RECURSIVE`) generates the last 7 dates, starting with one row and adding a day until today. Joining the loans to this calendar also shows days with no loans, which a plain `GROUP BY loaned_on` would skip.
- **Views** (`CREATE VIEW open_loans`) are stored queries. They hide joins and calculations; the program queries them like a table.
- **Dates**: `date('now')`, `date(x, '+14 days')`, `julianday(a) - julianday(b)` for the difference in days. ISO dates (`YYYY-MM-DD`) sort and compare correctly as text, which is why `CHECK (due_on >= loaned_on)` works. Note that `'now'` is UTC.

Money is stored as integer cents (`price_cents`), because floating point cannot represent amounts like 0.10 exactly. `price_cents * 110 / 100` is integer arithmetic.

## 12. JSON

SQLite has no JSON type but many JSON functions that work on `TEXT` columns. `CHECK (json_valid(tags))` makes sure only valid JSON is stored.

| SQL | Result |
|-----|--------|
| `json_each(tags)` | a table-valued function: one row per array element (`value`, `key`, `type`, …) |
| `json_insert(tags, '$[#]', 'gothic')` | appends to the array (`$[#]` = one past the end) |
| `tags ->> '$[0]'` | extracts as SQL value (`'classic'`); `->` would return JSON text (`'"classic"'`) |
| `json_object('title', title, ...)` | builds an object |
| `json_group_array(...)` | aggregate: collects values of all rows into an array |

In Rust, the array is converted with `serde_json` in `Book::from_row`. With the rusqlite feature `serde_json`, `serde_json::Value` can also be used directly as a parameter and column type.

If you often filter by a JSON value, an index on an expression helps: `CREATE INDEX ... ON books(tags ->> '$.lang')`. For lists that you search often, a separate table (`book_tags(book_id, tag)`) is usually the better design.

## 13. Full-text search with FTS5

`LIKE '%word%'` scans every row and knows nothing about words. FTS5 builds an inverted index (word → rows) and supports word queries, phrases, prefixes and ranking.

```sql
CREATE VIRTUAL TABLE books_fts USING fts5(
    title, summary,
    content = 'books', content_rowid = 'id',
    tokenize = 'porter unicode61');
```

- **External content** (`content = 'books'`): the index refers to the rows of `books` instead of storing a second copy of the text. The **triggers** `books_fts_insert`, `books_fts_delete` and `books_fts_update` keep it in sync; the special insert `INSERT INTO books_fts(books_fts, rowid, ...) VALUES ('delete', ...)` removes the old entry. `INSERT INTO books_fts(books_fts) VALUES ('rebuild')` re-indexes everything.
- **Tokenizer** `porter unicode61`: `unicode61` splits text into words and removes case and accents; `porter` reduces English words to their stem, so `travel` also finds `travels`.

Query syntax used in the example:

| Query | Finds |
|-------|-------|
| `travel` | the word (and, with Porter, `travels`, `travelled`, …) |
| `monster OR creature` | either word; `AND`, `NOT` and parentheses also work |
| `"natural selection"` | the exact phrase |
| `title:alice` | the word only in the column `title` |
| `roma*` | prefix search |

`bm25(books_fts)` is the relevance (smaller = better, so `ORDER BY score`); `highlight(books_fts, 1, '[', ']')` marks the hits in column 1 (`summary`). `snippet()` returns only the part of a long text around the hits. User input can contain invalid FTS syntax (`AND OR`); this is a normal SQLite error, so catch it, or quote the input as a phrase (`"` + input with `"` doubled + `"`).

## 14. BLOBs

`Vec<u8>` and `&[u8]` are stored as `BLOB` and read back the same way. `length(data)` returns the size in bytes without loading the data into Rust. A row can hold up to 1 GB by default, but BLOBs larger than a few hundred KB are often better kept as files with only the path in the database. For large BLOBs that must stay in the database, the `blob` feature offers incremental reading and writing (`conn.blob_open`) without loading everything into memory.

## 15. User-defined functions

```rust
conn.create_scalar_function("initials", 1, flags, |ctx| {
    let name: String = ctx.get(0)?;
    Ok(/* "Charles Darwin" -> "C.D." */)
})?;
```

The closure gets the arguments through `ctx.get(i)` (with the same type conversions as `row.get`) and returns any type that implements `ToSql`; returning `Option` produces `NULL`. `FunctionFlags::SQLITE_DETERMINISTIC` declares that the same input always gives the same output, which lets SQLite use the function in indexes and `CHECK` constraints. Functions are registered **per connection**: open a new connection, register them again (in `configure`, for example). Aggregate and window functions can be written too (`create_aggregate_function`, `create_window_function` with the `window` feature).

## 16. Performance: bulk inserts

Typical output (debug build, WSL2):

```text
   2000 rows, one commit per row:    269.7 ms  (      7417 rows/s)
 200000 rows, one transaction:       206.6 ms  (    968014 rows/s)
```

Every commit must write the journal and lock and unlock the database. In autocommit mode this happens for every row; in one transaction it happens once. The difference is about **100×** here, and even larger with `synchronous = FULL` or on slow disks, where each commit waits for an `fsync`. Further tips:

- Reuse a prepared statement (`prepare` once, `execute` many times) or `prepare_cached`.
- Use `cargo run --release` for measurements.
- For very large imports: create indexes after loading the data.
- Very large transactions are fine for SQLite; only the `-wal` file grows until the next checkpoint.

## 17. Threads and concurrency

- `Connection` is `Send` but not `Sync`: it can be moved into a thread, but not used by two threads at the same time. The standard pattern is **one connection per thread**, or a connection pool (`r2d2_sqlite`, `deadpool-sqlite`). In async code (tokio) use `tokio::task::spawn_blocking` or `tokio-rusqlite`, because rusqlite calls block.
- SQLite allows **many readers but one writer at a time**. In WAL mode readers keep working while someone writes, and see the state from the start of their read transaction.
- Writers wait for each other; `busy_timeout` controls how long before `SQLITE_BUSY` is returned. Keep write transactions short.

In `multiple_threads`, four threads each open a connection, call `configure` and insert 250 rows in `IMMEDIATE` transactions of 25 rows. The count at the end proves that no write was lost.

## 18. Indexes and query plans

`EXPLAIN QUERY PLAN <query>` shows how SQLite will run a query:

```text
SELECT title FROM books WHERE year BETWEEN 1850 AND 1870
    plan: SCAN books                                               <- reads every row
    plan: SEARCH books USING INDEX idx_books_year (year>? AND year<?)  <- after CREATE INDEX
SELECT count(*) FROM loans WHERE book_id = 3 AND returned_on IS NULL
    plan: SEARCH loans USING INDEX idx_loans_open (book_id=?)      <- partial index
```

- `SCAN` = full table scan, `SEARCH ... USING INDEX` = direct lookup.
- Index the columns used in `WHERE`, `JOIN ... ON` and `ORDER BY`. Foreign key columns are **not** indexed automatically (`idx_books_author`, `idx_loans_member`).
- A **partial index** (`WHERE returned_on IS NULL`) indexes only the rows that matter and stays small.
- Each index makes writes slower and the file bigger; add them where the query plan shows a `SCAN` on a large table.
- `PRAGMA optimize` updates the statistics the planner uses. Run it before closing a long-lived connection.

## 19. Backup and in-memory databases

**Do not copy the database file** with `cp` while it is in use: in WAL mode, recent changes are in `library.db-wal`, and a copy taken during a write can be inconsistent. Safe methods:

| Method | Notes |
|--------|-------|
| `conn.backup("main", path, progress)` | online backup API, page by page, works while other connections write; feature `backup` |
| `VACUUM INTO 'path'` | plain SQL; writes a compacted copy without free pages |
| `sqlite3 library.db ".backup copy.db"` | the same from the command line |

The example also restores the backup into an **in-memory database** (`Connection::open_in_memory()` + `restore`). In-memory databases are ideal for unit tests: every test gets a fresh, fast, isolated database that disappears when the connection is dropped. The output shows that changes to the copy do not affect the original.

The backup file is bigger than the compacted copy because it contains the free pages left by the dropped `numbers` table; `VACUUM` (or `VACUUM INTO`) removes them. `PRAGMA integrity_check` returns `ok` when the file is not corrupted.

## 20. Pitfalls

| Symptom | Cause and fix |
|---------|---------------|
| Foreign keys are not enforced | `PRAGMA foreign_keys = ON` is missing on this connection |
| `database is locked` | no `busy_timeout`, a long-running write transaction, or a `DEFERRED` transaction that read and then tried to write → use `Immediate` |
| `cannot move out of tx because it is borrowed` | a `Statement` from the transaction is still alive at `commit()` → put it in a `{ }` block |
| `Execute returned results` | `execute` used for a statement that returns rows → `query_row` / `query` |
| `InvalidColumnType` | `NULL` read into a non-`Option` type, or wrong type |
| `QueryReturnedNoRows` | `query_row` found nothing → `.optional()` |
| SQL injection | values put into SQL with `format!` → always use parameters |
| Wrong amounts | money stored as `REAL` → store cents as `INTEGER` |
| Dates off by hours | `'now'` is UTC; use `'localtime'` modifier for display only |
| Database file "too small" or copy incomplete | data is still in the `-wal` file → use the backup API or `VACUUM INTO` |

## 21. Ideas for experiments

1. Remove `PRAGMA foreign_keys = ON` and see which constraint errors in section 7 disappear.
2. Change the second `migrate` call into a migration 4 that adds `CREATE TABLE book_tags(book_id, tag)` and fills it from the JSON column with `INSERT INTO book_tags SELECT books.id, value FROM books, json_each(books.tags)`.
3. Add `PRAGMA synchronous = FULL` and compare the bulk insert timings.
4. Replace `TransactionBehavior::Immediate` in the worker threads by the default and increase the number of threads and rows; watch for `database is locked`.
5. Add `chrono` or `time` (rusqlite features of the same name) and read the date columns as typed dates.
6. Open `library.db` in the `sqlite3` shell while the program is not running and try `.schema`, `.tables`, and the queries from the code.
7. Write an aggregate function `median()` with `create_aggregate_function`.

## 22. Further reading

- [rusqlite documentation](https://docs.rs/rusqlite/)
- [SQLite documentation](https://sqlite.org/docs.html), especially [Datatypes](https://sqlite.org/datatype3.html), [STRICT tables](https://sqlite.org/stricttables.html), [WAL](https://sqlite.org/wal.html), [Foreign keys](https://sqlite.org/foreignkeys.html), [Query planner](https://sqlite.org/queryplanner.html)
- [FTS5](https://sqlite.org/fts5.html), [JSON functions](https://sqlite.org/json1.html), [Window functions](https://sqlite.org/windowfunctions.html), [Date and time functions](https://sqlite.org/lang_datefunc.html)
- [Appropriate uses for SQLite](https://sqlite.org/whentouse.html)
