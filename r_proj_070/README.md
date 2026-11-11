# SQLite in Rust with rusqlite

A comprehensive, runnable example of using the embedded database [SQLite](https://sqlite.org)
from Rust with the crate [`rusqlite`](https://crates.io/crates/rusqlite). The program builds a
small library database (authors, books, members, loans) and goes through the features you
need in real applications, from the first `CREATE TABLE` to backups.

For a step-by-step explanation of the code, see [DOCUMENTATION.md](DOCUMENTATION.md).

```text
authors 1──n books 1──n loans n──1 members
                │  1──1 covers (BLOB)
                │  1──n page_views
                └── books_fts (FTS5 full-text index, kept in sync by triggers)
```

## Running

```bash
cargo run
sqlite3 library.db          # optional: look at the result with the SQLite shell
```

SQLite is compiled into the program (feature `bundled`), so you need only a C compiler
(`gcc` or `clang`), no installed SQLite library. The first build takes about a minute.

Every run deletes and recreates `library.db`, so the output is always the same (except for dates
and timings).

## What the example shows

| # | Topic | Main APIs and SQL |
|---|-------|-------------------|
| 1 | Opening a database, PRAGMAs | `Connection::open`, `journal_mode = WAL`, `foreign_keys = ON`, `busy_timeout` |
| 2 | Schema migrations | `PRAGMA user_version`, transactional DDL, `STRICT` tables |
| 3 | Inserting rows | `prepare`, `params!`, `named_params!`, `RETURNING`, `last_insert_rowid` |
| 4 | Queries | `query_row`, `.optional()`, `query_map` into structs, `Option` for NULL, `ToSql`/`FromSql` for an enum, `params_from_iter`, escaping `LIKE` |
| 5 | Changing data | `UPDATE` with row count, UPSERT (`ON CONFLICT DO UPDATE`), `ON DELETE CASCADE` |
| 6 | Transactions | business rules in an `IMMEDIATE` transaction, rollback on early return, savepoints |
| 7 | Errors | telling UNIQUE, FOREIGN KEY, CHECK, NOT NULL and type errors apart |
| 8 | Reports | `LEFT JOIN`, `GROUP BY`, `FILTER`, window functions, recursive CTE, views, date functions |
| 9 | JSON | `json_each`, `json_insert`, `->>`, `json_group_array` |
| 10 | Full-text search | FTS5 with Porter stemmer, `MATCH`, `bm25`, `highlight` |
| 11 | BLOBs | storing and reading binary data |
| 12 | Rust functions in SQL | `create_scalar_function` |
| 13 | Bulk inserts | one commit per row vs. one transaction (about 100× faster) |
| 14 | Threads | one connection per thread, WAL, busy timeout |
| 15 | Indexes | `EXPLAIN QUERY PLAN`, partial index, `PRAGMA optimize` |
| 16 | Backup | online backup API, in-memory database, `VACUUM INTO`, `integrity_check` |

## Files the program creates

| File | Content |
|------|---------|
| `library.db` | the database |
| `library.db-wal`, `library.db-shm` | write-ahead log and its index (WAL mode); part of the database while it is open |
| `library-backup.db` | copy made with the backup API |
| `library-compact.db` | compacted copy made with `VACUUM INTO` |

They are listed in `.gitignore`.

## Dependencies

```toml
anyhow = "1"
rusqlite = { version = "0.42", features = ["bundled", "functions", "backup"] }
serde_json = "1"
```
