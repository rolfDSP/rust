# Task Microservice

An example of a web-based microservice in Rust, built with [axum](https://github.com/tokio-rs/axum) and [Tokio](https://tokio.rs). It provides a JSON REST API for managing a list of tasks.

The domain is deliberately simple, so the example can focus on what most microservices need:

- REST endpoints with JSON requests and responses
- State shared safely between concurrent requests
- A central error type that gives clients consistent JSON errors
- Input validation
- A health check endpoint
- Request logging and request timeouts as middleware
- Configuration through environment variables
- Graceful shutdown on Ctrl+C and SIGTERM
- Endpoint tests that need no network port

Tasks are stored in memory, so **all data is lost when the service stops**. See [Limitations](#limitations) before using this code as the basis for a production service.

## Contents

- [Quick start](#quick-start)
- [Configuration](#configuration)
- [API reference](#api-reference)
- [Error handling](#error-handling)
- [Project structure](#project-structure)
- [How it works](#how-it-works)
- [Testing](#testing)
- [Dependencies](#dependencies)
- [Extending the service](#extending-the-service)
- [Limitations](#limitations)

## Quick start

You need a Rust toolchain with support for the 2024 edition (Rust 1.85 or newer). The project was last built and tested with Rust 1.99.

Start the service:

```bash
cargo run
```

It listens on `http://127.0.0.1:3000`. Try it from a second terminal:

```bash
# Check that the service is up
curl localhost:3000/health

# Create a task
curl -X POST localhost:3000/tasks \
  -H 'Content-Type: application/json' \
  -d '{"title":"Learn axum"}'

# Mark it as completed
curl -X PATCH localhost:3000/tasks/1 \
  -H 'Content-Type: application/json' \
  -d '{"completed":true}'

# List all tasks
curl localhost:3000/tasks

# Delete it
curl -X DELETE localhost:3000/tasks/1
```

Stop the service with Ctrl+C.

For an optimized build:

```bash
cargo build --release
./target/release/r_proj_064
```

## Configuration

All settings are optional environment variables.

| Variable | Default | Description |
|---|---|---|
| `BIND_ADDR` | `127.0.0.1:3000` | IP address and port to listen on. Use `0.0.0.0:3000` to accept connections from other machines, or from outside a container. |
| `REQUEST_TIMEOUT_SECS` | `10` | Maximum time for a request, in whole seconds. |
| `RUST_LOG` | `info,tower_http=debug` | Which log messages to print, in the [`EnvFilter` syntax](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html). |

```bash
BIND_ADDR=0.0.0.0:8080 REQUEST_TIMEOUT_SECS=30 RUST_LOG=warn cargo run
```

If `BIND_ADDR` or `REQUEST_TIMEOUT_SECS` has an invalid value, the service exits with code 1 at startup instead of silently using the default:

```text
Error: "environment variable BIND_ADDR has invalid value 'nonsense'"
```

With the default log level, each request is logged like this:

```text
DEBUG request{method=POST uri=/tasks version=HTTP/1.1}: tower_http::trace::on_request: started processing request
DEBUG request{method=POST uri=/tasks version=HTTP/1.1}: tower_http::trace::on_response: finished processing request latency=0 ms status=201
```

## API reference

| Method | Path | Purpose | Success status |
|---|---|---|---|
| `GET` | `/health` | Check that the service is up | `200 OK` |
| `GET` | `/tasks` | List all tasks | `200 OK` |
| `POST` | `/tasks` | Create a task | `201 Created` |
| `GET` | `/tasks/{id}` | Get one task | `200 OK` |
| `PATCH` | `/tasks/{id}` | Change a task | `200 OK` |
| `DELETE` | `/tasks/{id}` | Delete a task | `204 No Content` |

Requests with a body must send the header `Content-Type: application/json`. A body can be at most 2 MB, which is axum's default limit.

### The task object

```json
{ "id": 1, "title": "Learn axum", "completed": false }
```

| Field | Type | Description |
|---|---|---|
| `id` | unsigned integer | Assigned by the service, starting at 1. The ids of deleted tasks are not reused. |
| `title` | string | Must not be empty or only whitespace. Leading and trailing whitespace is removed. |
| `completed` | boolean | `false` for a new task. |

### `GET /health`

Intended for load balancers, Docker, and Kubernetes. It only shows that the service can answer requests.

```json
{ "status": "ok" }
```

### `GET /tasks`

Returns all tasks as an array sorted by `id`, or an empty array if there are none. There is no pagination or filtering.

```json
[
  { "id": 1, "title": "Learn axum", "completed": true },
  { "id": 2, "title": "Write docs", "completed": false }
]
```

### `POST /tasks`

Request body:

```json
{ "title": "Learn axum" }
```

Response, with status `201 Created`:

```json
{ "id": 1, "title": "Learn axum", "completed": false }
```

### `GET /tasks/{id}`

Returns the task, or `404 Not Found` if the id is unknown.

### `PATCH /tasks/{id}`

Changes only the fields in the request body and returns the updated task. Both fields are optional:

```json
{ "title": "Renamed", "completed": true }
```

Some details of this endpoint can be surprising:

- A field left out or set to `null` keeps its current value, so `{}` is a valid request that changes nothing and returns `200 OK`.
- Unknown fields are ignored. A misspelled field, as in `{"titel":"x"}`, also changes nothing and returns `200 OK`.
- The title is validated before the task is looked up, so a blank title sent to an unknown id returns `422`, not `404`.

### `DELETE /tasks/{id}`

Deletes the task and returns `204 No Content` with an empty body, or `404 Not Found` if the id is unknown. Deleting the same task twice therefore returns `404` the second time.

## Error handling

Errors from the task endpoints have a JSON body with a single `error` field:

```json
{ "error": "task 99 not found" }
```

| Status | When | Example message |
|---|---|---|
| `400 Bad Request` | The id in the path is not an unsigned integer | ``Cannot parse `abc` to a `u64` `` |
| `400 Bad Request` | The body is not valid JSON | `Failed to parse the request body as JSON: ...` |
| `404 Not Found` | No task has this id | `task 99 not found` |
| `413 Payload Too Large` | The body is larger than 2 MB | `Failed to buffer the request body: length limit exceeded` |
| `415 Unsupported Media Type` | The `Content-Type` header is missing or wrong | ``Expected request with `Content-Type: application/json` `` |
| `422 Unprocessable Entity` | A field is missing or has the wrong type | ``Failed to deserialize the JSON body into the target type: missing field `title` ...`` |
| `422 Unprocessable Entity` | The title is empty or only whitespace | `title must not be empty` |

Clients should rely on the status code, not the message text. Most of these messages come from axum and may change with a new axum version.

Three responses come from outside the handlers and have an **empty body** instead of the JSON format:

| Status | When |
|---|---|
| `404 Not Found` | The path does not exist, such as `/nope` |
| `405 Method Not Allowed` | The path exists but not with this method, such as `PUT /tasks/1` |
| `504 Gateway Timeout` | The request took longer than `REQUEST_TIMEOUT_SECS` |

## Project structure

```text
r_proj_064/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs      Startup, logging, and graceful shutdown
│   ├── lib.rs       Declares the library modules
│   ├── config.rs    Reads the environment variables
│   ├── routes.rs    Routes, handlers, and middleware
│   ├── models.rs    JSON data types
│   ├── store.rs     In-memory task storage
│   └── error.rs     Maps errors to HTTP responses
└── tests/
    └── api.rs       Endpoint tests
```

The service lives in a library crate, and `main.rs` is a thin binary that starts it. Integration tests in `tests/` can only use a library, so this split is what lets them build the router directly.

## How it works

### Request flow

A request passes through these layers:

```text
Client
  │
  ▼
TraceLayer       logs the request and response
  │
  ▼
TimeoutLayer     answers with 504 if the request takes too long
  │
  ▼
Router           finds the handler for the method and path
  │
  ▼
Handler          parses the input and validates it
  │
  ▼
TaskStore        reads or changes the data
```

The result travels back through the same layers. With axum's `Router::layer`, the layer added last runs first. `TraceLayer` is added last so it also logs the responses of requests that timed out.

### Shared state

`TaskStore` holds an `Arc<RwLock<...>>`, so cloning it is cheap and all clones see the same data. axum gives each handler a clone through the `State` extractor:

```rust
async fn list_tasks(State(store): State<TaskStore>) -> Json<Vec<Task>> {
    Json(store.list())
}
```

The lock is `std::sync::RwLock`, not the Tokio version. The standard library lock is the better choice when a lock is never held across an `.await`, which is the case for every `TaskStore` method. Many readers can run at the same time, and a writer gets exclusive access.

### Errors

Handlers return `Result<_, AppError>`. Because `AppError` implements `IntoResponse`, axum can turn it into an HTTP response, and handlers can use `?` to return early.

By default, axum answers malformed input with a plain text message before the handler runs. To return JSON errors instead, handlers take the extractor wrapped in a `Result` and convert the failure into an `AppError`:

```rust
async fn create_task(
    State(store): State<TaskStore>,
    body: Result<Json<CreateTask>, JsonRejection>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    let Json(input) = body?;
    let task = store.create(validate_title(&input.title)?);
    Ok((StatusCode::CREATED, Json(task)))
}
```

### Graceful shutdown

On Ctrl+C or SIGTERM, the signal Docker and Kubernetes send to stop a container, the service stops accepting new connections and lets running requests finish before it exits with code 0. SIGTERM handling is compiled only on Unix systems. Other systems use Ctrl+C alone.

## Testing

```bash
cargo test
```

The tests in `tests/api.rs` send requests straight to the router with `tower::ServiceExt::oneshot`, so they need no network port and run in milliseconds. Each test creates its own store, so tests can run in parallel without affecting each other.

| Test | What it checks |
|---|---|
| `health_reports_ok` | The health endpoint answers |
| `task_lifecycle` | Create, update, read, delete, and list, including trimming of the title |
| `tasks_are_listed_by_id` | The list order |
| `deleted_ids_are_not_reused` | A new task gets a new id after a delete |
| `unknown_task_gives_404_on_every_method` | `GET`, `PATCH`, and `DELETE` with an unknown id |
| `blank_title_is_rejected_and_changes_nothing` | Validation on `POST` and `PATCH` |
| `malformed_requests_get_json_errors` | Invalid JSON, a missing field, and an invalid id |
| `slow_requests_time_out` | The timeout middleware, using a deliberately slow route |

These parts have no automated tests: `config.rs`, the logging setup, and the shutdown handling in `main.rs`.

To check code style and common mistakes:

```bash
cargo fmt --check
cargo clippy --all-targets
```

## Dependencies

| Crate | Purpose |
|---|---|
| [`axum`](https://docs.rs/axum) | Web framework: routing, extractors, and responses |
| [`tokio`](https://docs.rs/tokio) | Async runtime, network sockets, and signal handling |
| [`tower-http`](https://docs.rs/tower-http) | Middleware for tracing and timeouts |
| [`serde`](https://docs.rs/serde), [`serde_json`](https://docs.rs/serde_json) | JSON conversion |
| [`thiserror`](https://docs.rs/thiserror) | Deriving the error type |
| [`tracing`](https://docs.rs/tracing), [`tracing-subscriber`](https://docs.rs/tracing-subscriber) | Structured logging |

Used only by the tests:

| Crate | Purpose |
|---|---|
| [`tower`](https://docs.rs/tower) | `oneshot`, to send a single request to the router |
| [`http-body-util`](https://docs.rs/http-body-util) | Reading response bodies |

## Extending the service

### Adding a field to a task

1. Add the field to `Task` in `src/models.rs`, and to `CreateTask` or `UpdateTask` if clients can set it.
2. Use it in `create` and `update` in `src/store.rs`.
3. Add any validation to the handlers in `src/routes.rs`.
4. Extend the tests in `tests/api.rs`, which compare complete task objects.

### Adding an endpoint

1. Write a handler function in `src/routes.rs`.
2. Register it in `api_routes`.
3. For a new kind of failure, add a variant to `AppError` in `src/error.rs` and give it a status code in `into_response`.

### Using a database

Handlers only use the `TaskStore` methods, so a database mainly changes `src/store.rs`. With a crate such as [`sqlx`](https://docs.rs/sqlx), `TaskStore` would hold a connection pool, and its methods would become `async` and return a `Result`. The handlers would then need to `.await` those calls and map database failures to a new `AppError` variant.

## Limitations

This is a learning example. A production service would need more work in these areas:

- **Persistence:** all data is kept in memory and lost on shutdown. Running several instances would give each one separate data.
- **Security:** there is no authentication, authorization, TLS, or rate limiting. Anyone who can reach the port can read and change all tasks, so take care when binding to `0.0.0.0`.
- **Browser clients:** CORS is not configured, so web pages on other origins cannot call the API.
- **Input limits:** the title has no maximum length other than the 2 MB body limit.
- **Lists:** `GET /tasks` always returns everything, with no pagination.
- **Health check:** `/health` does not check any dependencies, because the service has none yet.
- **Observability:** there are logs, but no metrics or distributed tracing.
- **API documentation:** there is no OpenAPI specification.
