# Code Documentation

This document explains how the code of the task microservice works, file by file. It covers what each part does and why it is written that way, including the Rust and axum concepts involved.

For how to run and call the service, see [README.md](README.md).

## Contents

1. [The big picture](#1-the-big-picture)
2. [How the crate is organized](#2-how-the-crate-is-organized)
3. [Startup: `main.rs`](#3-startup-mainrs)
4. [Settings: `config.rs`](#4-settings-configrs)
5. [Data types: `models.rs`](#5-data-types-modelsrs)
6. [Storage: `store.rs`](#6-storage-storers)
7. [Errors: `error.rs`](#7-errors-errorrs)
8. [The HTTP layer: `routes.rs`](#8-the-http-layer-routesrs)
9. [Following requests through the code](#9-following-requests-through-the-code)
10. [Tests: `tests/api.rs`](#10-tests-testsapirs)
11. [Concept summary](#11-concept-summary)

## 1. The big picture

The program is a web server that keeps a list of tasks in memory. Clients send HTTP requests to create, read, change, and delete tasks, and the server answers with JSON.

Each source file has one responsibility:

| File | Question it answers |
|---|---|
| `main.rs` | How does the program start and stop? |
| `config.rs` | Which settings does it run with? |
| `routes.rs` | What happens with an incoming HTTP request? |
| `models.rs` | What does the data look like? |
| `store.rs` | Where is the data kept? |
| `error.rs` | How does a failure become an HTTP response? |

The files depend on each other in one direction, from the HTTP layer down to the data:

```text
main.rs ──► config.rs
   │
   ▼
routes.rs ──► error.rs
   │
   ├────────► store.rs ──► models.rs
   │
   └────────► models.rs
```

`store.rs` knows nothing about HTTP. It has no status codes, no JSON, and no axum types. Only `routes.rs` and `error.rs` deal with HTTP. That separation is what would make it possible to replace the storage without rewriting the handlers.

## 2. How the crate is organized

Cargo finds both `src/lib.rs` and `src/main.rs` automatically and builds them as two separate crates that share a name:

- **The library** (`lib.rs`) contains the whole service.
- **The binary** (`main.rs`) uses the library and starts it.

`lib.rs` contains only documentation and module declarations:

```rust
pub mod config;
pub mod error;
pub mod models;
pub mod routes;
pub mod store;
```

Each `pub mod` line tells the compiler to include the file of that name and makes it visible outside the library.

Because the binary is a separate crate, `main.rs` imports these modules through the crate name instead of `crate::`:

```rust
use r_proj_064::config::Config;
```

The reason for this split is testing. The files in `tests/` are also separate crates, and they can only import from a library, not from a binary. With the split, the tests can build the same router that the real service uses.

## 3. Startup: `main.rs`

### The `main` function

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
```

Rust has no built-in runtime for `async` code, so a plain `main` cannot be `async`. The `#[tokio::main]` macro rewrites this function into a normal `main` that creates a Tokio runtime and runs the async body on it. By default, this runtime uses several threads, so requests can really run in parallel.

The return type `Result<(), Box<dyn Error>>` means "nothing on success, or any kind of error". This allows `?` to be used on the different error types in this function. When `main` returns an error, Rust prints it and the program exits with code 1.

### Step 1: Set up logging

```rust
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
```

`EnvFilter` decides which log messages are printed. `try_from_default_env` reads the `RUST_LOG` variable and returns an error if it can't be used, for example because it isn't set. `unwrap_or_else` then supplies the default: `info` level in general, and the more detailed `debug` level for `tower_http`, which is the crate that logs each request.

`tracing_subscriber::fmt()` creates a logger that writes formatted lines to the terminal, and `init()` installs it for the whole program. From here on, calls like `tracing::info!(...)` produce output. Before this point, they would print nothing, which is why logging is set up first.

### Step 2: Load the settings and build the service

```rust
    let config = Config::from_env()?;
    let app = routes::app(TaskStore::new(), config.request_timeout);
```

`Config::from_env` returns `Result<Config, String>`. If it returns an error, `?` leaves `main` immediately. Rust converts the `String` into a `Box<dyn Error>` automatically. Rust prints errors returned from `main` in their debug format, which is why the message appears in quotes:

```text
Error: "environment variable BIND_ADDR has invalid value 'nonsense'"
```

`routes::app` then creates the `Router`, the object that represents the whole service. Nothing is listening yet.

### Step 3: Open the network port

```rust
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
```

`bind` reserves the address and port. It can fail, for example if another program already uses the port, and `?` then ends the program with that error.

### Step 4: Serve until told to stop

```rust
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
```

`axum::serve` connects the listener to the router: it accepts connections and passes their requests to the router. The program stays at this `.await` for as long as the server runs.

`with_graceful_shutdown` takes a future, which is a value representing work that finishes at some later time. When that future completes, the server stops accepting new connections, waits for running requests to finish, and only then does the `.await` return. `main` then logs a final message and returns `Ok(())`, giving exit code 0.

### Waiting for a stop signal

```rust
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("cannot listen for Ctrl+C");
    };
```

This function completes as soon as the process is told to stop. There are two possible signals, and each gets its own future. Creating an `async` block doesn't run it. It only describes the work, which starts when something awaits it.

The second future handles SIGTERM, the signal Docker and Kubernetes send to stop a service:

```rust
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("cannot listen for SIGTERM")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
```

`#[cfg(...)]` chooses code at compile time, so only one of these two statements exists in the compiled program. SIGTERM exists only on Unix systems. On other systems, `pending` provides a future that never completes, so the code below works unchanged and only Ctrl+C can end the wait.

```rust
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
```

`tokio::select!` waits on both futures at once and continues as soon as the first one completes. Whichever signal arrives first ends the wait, and the function returns.

## 4. Settings: `config.rs`

```rust
#[derive(Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub request_timeout: Duration,
}
```

The fields use specific types, not strings. Once a `Config` exists, its values are known to be valid, and the rest of the program doesn't need to check them again.

```rust
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            bind_addr: read_var("BIND_ADDR", SocketAddr::from(([127, 0, 0, 1], 3000)))?,
            request_timeout: Duration::from_secs(read_var("REQUEST_TIMEOUT_SECS", 10)?),
        })
    }
```

Each `read_var` call either produces a value or leaves the function with an error through `?`.

The helper function `read_var` is generic, so it works for any type that can be parsed from a string:

```rust
fn read_var<T: FromStr>(name: &str, default: T) -> Result<T, String> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("environment variable {name} has invalid value '{value}'")),
        Err(env::VarError::NotPresent) => Ok(default),
```

The compiler works out `T` from how each call is used. `SocketAddr::from(...)` makes `T` a `SocketAddr` in the first call. In the second, `Duration::from_secs` needs a `u64`, so `T` is `u64` and the literal `10` becomes a `u64` as well. `value.parse()` then uses the parser of that type.

The `match` has three outcomes:

| `env::var` result | Meaning | What `read_var` does |
|---|---|---|
| `Ok(value)` | The variable is set | Parses it, and returns an error if parsing fails |
| `Err(NotPresent)` | The variable is not set | Returns the default |
| `Err(NotUnicode(_))` | The value isn't valid text | Returns an error |

An unset variable and an invalid one are treated differently on purpose. If a typo fell back to the default, the service would appear to work while running with the wrong settings.

## 5. Data types: `models.rs`

This file defines the JSON formats of the API as plain structs.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub completed: bool,
}
```

`#[derive(...)]` makes the compiler generate standard functionality:

| Derive | What it adds |
|---|---|
| `Debug` | Printing the value for debugging |
| `Clone` | Making a copy with `.clone()`, which the store needs to hand out tasks |
| `Serialize` | Conversion to JSON, for responses |
| `Deserialize` | Conversion from JSON |
| `PartialEq` | Comparison with `==` |

The current code only needs `Debug`, `Clone`, and `Serialize` on `Task`. The tests compare JSON values, not `Task` structs, so `PartialEq` and `Deserialize` are not used yet.

Each input format has its own struct, separate from `Task`:

```rust
pub struct CreateTask {
    pub title: String,
}
```

```rust
pub struct UpdateTask {
    pub title: Option<String>,
    pub completed: Option<bool>,
}
```

Separate structs define exactly what a client may send. A client can't choose the `id`, because `CreateTask` and `UpdateTask` have no such field. These structs derive only `Deserialize`, since they are only ever read from requests.

Their fields control what happens with missing input:

- `CreateTask.title` is a `String`, so it is required. If it's missing, parsing fails and the client gets a `422`.
- The fields of `UpdateTask` are `Option`s, so they are optional. Serde sets a missing field or a JSON `null` to `None`, and the store treats `None` as "keep the current value".

Serde ignores JSON fields the struct doesn't have, which is why a misspelled field in a `PATCH` request changes nothing and causes no error.

## 6. Storage: `store.rs`

### The structure

```rust
#[derive(Default)]
struct Inner {
    last_id: u64,
    // A BTreeMap keeps the tasks sorted by id, so listings have a stable order.
    tasks: BTreeMap<u64, Task>,
}
```

`Inner` holds the actual data. `#[derive(Default)]` provides an initial value with `last_id` at 0 and an empty map.

`last_id` is separate from the map so that ids are never reused. If the next id were calculated from the highest id in the map, deleting the newest task would make its id available again, and a client still holding the old id would then reach a different task.

```rust
#[derive(Clone, Default)]
pub struct TaskStore {
    inner: Arc<RwLock<Inner>>,
}
```

Each layer of `Arc<RwLock<Inner>>` solves one problem:

- **`RwLock`** makes access safe from several threads. Any number of threads can read at once, but a writer gets exclusive access. Rust won't compile code that shares changeable data between threads without something like this.
- **`Arc`** ("atomically reference counted") lets several owners share one value. Cloning an `Arc` doesn't copy the data. It creates another pointer to the same data and increases a counter. The data is freed when the last owner is gone.

`#[derive(Clone)]` on `TaskStore` therefore clones only the `Arc`. Every clone refers to the same tasks, which is exactly what is needed, because each request's handler gets its own clone. The field is private, so all access has to go through the methods below.

### Reading

```rust
    pub fn get(&self, id: u64) -> Option<Task> {
        let inner = self.inner.read().expect("task store lock poisoned");
        inner.tasks.get(&id).cloned()
    }
```

`read()` waits for the lock and returns a guard, a value that gives access to the data. The lock is released automatically when the guard goes out of scope at the end of the function, so it can't be forgotten.

The map's `get` returns `Option<&Task>`, a reference into the map. That reference can't outlive the guard, because the lock is released when the function ends. `.cloned()` therefore makes an independent copy to return. `list` works the same way for all tasks:

```rust
        inner.tasks.values().cloned().collect()
```

`.expect(...)` deals with lock poisoning. If a thread panics while holding a lock, the standard library marks the lock as poisoned, because the data may be half-changed. Later attempts to lock it return an error, and `expect` turns that error into a panic. None of the code that holds the lock is expected to panic, so this shouldn't happen in practice.

### Writing

```rust
    pub fn create(&self, title: String) -> Task {
        let mut inner = self.inner.write().expect("task store lock poisoned");
        inner.last_id += 1;
        let task = Task {
            id: inner.last_id,
            title,
            completed: false,
        };
        inner.tasks.insert(task.id, task.clone());
        task
    }
```

`write()` waits until no one else holds the lock. Note that the method takes `&self`, not `&mut self`, and can still change data. The `RwLock` provides this "interior mutability" by checking the access rules at run time, which is what makes shared changes possible.

The lock is held for the whole function, so increasing the counter and inserting the task happen as one step. Two simultaneous requests can't get the same id.

```rust
    pub fn update(&self, id: u64, title: Option<String>, completed: Option<bool>) -> Option<Task> {
        let mut inner = self.inner.write().expect("task store lock poisoned");
        let task = inner.tasks.get_mut(&id)?;
```

`get_mut` returns an `Option` with a changeable reference. Here `?` works on an `Option`: if the task doesn't exist, the function returns `None` immediately.

```rust
        if let Some(title) = title {
            task.title = title;
        }
```

`if let` runs only if the field was given, and otherwise leaves the old value in place. This is where the "keep the current value" behavior of `PATCH` comes from.

```rust
    pub fn delete(&self, id: u64) -> bool {
        let mut inner = self.inner.write().expect("task store lock poisoned");
        inner.tasks.remove(&id).is_some()
    }
```

`remove` returns the removed value, or `None` if there was none, so `is_some()` tells the caller whether the task existed.

### Why the standard library lock?

Tokio has its own `RwLock`, but this code uses `std::sync::RwLock`. The difference matters only when a lock is held across an `.await`, because a task that holds a standard lock while paused could block other work. Here the methods aren't `async`, so that can't happen, and the locks are held only for very short operations. In that situation the simpler standard lock is the recommended choice.

## 7. Errors: `error.rs`

### Defining the errors

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("task {0} not found")]
    TaskNotFound(u64),

    #[error("{0}")]
    Validation(String),

    /// The body was not valid JSON, or did not match the expected type.
    #[error(transparent)]
    InvalidJson(#[from] JsonRejection),

    /// A path parameter could not be parsed, for example `/tasks/abc`.
    #[error(transparent)]
    InvalidPath(#[from] PathRejection),
}
```

An `enum` lists every way a request can fail. The `thiserror` crate generates the boilerplate from the attributes:

| Attribute | What it generates |
|---|---|
| `#[error("task {0} not found")]` | The error message, where `{0}` is the first field of the variant |
| `#[error(transparent)]` | Uses the message of the wrapped error unchanged |
| `#[from]` | A conversion from the wrapped type to `AppError`, which lets `?` work |

The `#[from]` conversions are what make `let Json(input) = body?;` work in the handlers: `?` converts a `JsonRejection` into an `AppError` automatically.

### Turning an error into a response

```rust
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::TaskNotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::InvalidJson(rejection) => rejection.status(),
            AppError::InvalidPath(rejection) => rejection.status(),
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
```

`IntoResponse` is axum's trait for "can become an HTTP response". By implementing it, `AppError` can be returned directly from any handler.

The two groups of errors get their status differently. For this service's own errors, the code chooses the status. Axum's rejections already know the right status, such as `400`, `415`, or `422`, so `rejection.status()` asks them.

The `match` must cover every variant, so the compiler reports an error if a new variant is added without a status. The last line builds the body with the `json!` macro and uses the fact that a tuple of status and body also implements `IntoResponse`.

## 8. The HTTP layer: `routes.rs`

### Building the router

```rust
pub fn app(store: TaskStore, request_timeout: Duration) -> Router {
    with_middleware(api_routes(store), request_timeout)
}
```

The router is built in two steps. They are separate functions so the timeout test can apply the same middleware to its own slow route.

```rust
fn api_routes(store: TaskStore) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tasks", get(list_tasks).post(create_task))
        .route(
            "/tasks/{id}",
            get(get_task).patch(update_task).delete(delete_task),
        )
        .with_state(store)
}
```

Each `route` call connects a path to handlers for different HTTP methods. `{id}` marks a part of the path that the handler can read.

`with_state(store)` gives the router the store that handlers can request. Rust checks this at compile time: a handler that asks for a state type the router doesn't have won't compile.

### Middleware

```rust
pub fn with_middleware(router: Router, request_timeout: Duration) -> Router {
    router
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ))
        .layer(TraceLayer::new_for_http())
}
```

Middleware is code that wraps the handlers and runs for each request. Each `.layer` call wraps everything added so far, so the last layer ends up on the outside and sees the request first:

```text
TraceLayer ( TimeoutLayer ( routes ) )
```

- **`TimeoutLayer`** runs the inner service with a time limit. If the limit expires, it stops waiting and answers with a `504` and an empty body. Async Rust makes this possible by dropping the unfinished future, which cancels the work at its current `.await`.
- **`TraceLayer`** logs when a request starts and finishes. Being on the outside, it also logs the `504` responses created by the timeout.

### How handlers work

A handler is a normal `async` function. Axum looks at its parameter types to decide what to extract from the request, and at its return type to decide how to build the response. There is no registration of parameters and no manual parsing.

The simplest handler takes nothing:

```rust
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
```

`Json` is used in both directions. As a return value, it converts data to JSON and sets the `Content-Type` header. As a parameter, it parses the request body.

```rust
async fn list_tasks(State(store): State<TaskStore>) -> Json<Vec<Task>> {
    Json(store.list())
}
```

`State<TaskStore>` is an extractor that provides a clone of the store. The parameter uses pattern matching: `State(store)` unpacks the wrapper right in the parameter list, so the function body works directly with `store`.

### Receiving parsing failures

Normally, a handler declares `Json<CreateTask>`, and if parsing fails, axum answers with a plain text message without calling the handler. This service wants JSON errors everywhere, so its handlers wrap the extractors in a `Result`:

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

The handler now always runs, and receives a parsing failure as an ordinary `Err` value. `body?` converts it into an `AppError` through the generated `From` conversion and returns it.

The order of the parameters matters. An extractor that reads the request body must come last, because the body can only be read once. Axum enforces this at compile time.

On success, the handler returns a tuple of status and body, which sets the status to `201 Created` instead of the default `200 OK`.

### Reading one task

```rust
async fn get_task(
    State(store): State<TaskStore>,
    id: Result<Path<u64>, PathRejection>,
) -> Result<Json<Task>, AppError> {
    let Path(id) = id?;
    store.get(id).map(Json).ok_or(AppError::TaskNotFound(id))
}
```

`Path<u64>` parses the `{id}` part of the path as a number, so `/tasks/abc` and `/tasks/-1` fail with a `400`.

The last line chains two conversions:

| Step | Type afterwards |
|---|---|
| `store.get(id)` | `Option<Task>` |
| `.map(Json)` | `Option<Json<Task>>`, since `Json` can be used like a function that wraps a value |
| `.ok_or(AppError::TaskNotFound(id))` | `Result<Json<Task>, AppError>`, where `None` becomes the error |

### Changing a task

```rust
    let Path(id) = id?;
    let Json(input) = body?;
    let title = input.title.as_deref().map(validate_title).transpose()?;
```

The third line validates a title only if one was given:

| Step | Type afterwards |
|---|---|
| `input.title` | `Option<String>` |
| `.as_deref()` | `Option<&str>`, which borrows the text |
| `.map(validate_title)` | `Option<Result<String, AppError>>` |
| `.transpose()` | `Result<Option<String>, AppError>` |
| `?` | `Option<String>`, or an early return with the error |

Validation happens here, before the store is called. That's why a blank title sent to an unknown id returns `422` and not `404`.

### Deleting a task

```rust
    if store.delete(id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::TaskNotFound(id))
    }
```

A `StatusCode` alone also implements `IntoResponse`, and produces a response with an empty body.

### Validation

```rust
fn validate_title(title: &str) -> Result<String, AppError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title must not be empty".to_string()));
    }
    Ok(title.to_string())
}
```

The function returns the cleaned-up title, not just a yes or no. A caller therefore can't accidentally store the untrimmed input after validating it. Both `create_task` and `update_task` use it, so the rules are in one place.

## 9. Following requests through the code

### A successful request

Here is what happens for `POST /tasks` with the body `{"title":"  Learn axum "}`:

1. **Connection:** `axum::serve` accepts the connection and runs it as a Tokio task, so one connection doesn't block others.
2. **`TraceLayer`** logs `started processing request`.
3. **`TimeoutLayer`** starts its timer.
4. **The router** matches `POST` and `/tasks` and selects `create_task`.
5. **Extractors:** axum clones the store for `State`. For `Json`, it checks the `Content-Type` header, reads the body, and parses it into a `CreateTask`.
6. **`create_task`** unpacks the body. `validate_title` returns `"Learn axum"`, without the extra spaces.
7. **`TaskStore::create`** takes the write lock, assigns id 1, stores the task, and returns a copy. The lock is released when the method returns.
8. **The return value** `(StatusCode::CREATED, Json(task))` becomes an HTTP response through `IntoResponse`.
9. **`TraceLayer`** logs `finished processing request latency=0 ms status=201`.

The client receives:

```text
HTTP/1.1 201 Created
content-type: application/json

{"id":1,"title":"Learn axum","completed":false}
```

### A failing request

Here is `GET /tasks/99` when no such task exists:

1. The middleware and router work as above, and the router selects `get_task`.
2. `Path<u64>` parses `99` successfully.
3. `store.get(99)` takes the read lock and returns `None`.
4. `.ok_or(...)` turns that into `Err(AppError::TaskNotFound(99))`.
5. Axum calls `AppError::into_response`, which selects `404` and creates the JSON body from the error message.

The client receives:

```text
HTTP/1.1 404 Not Found
content-type: application/json

{"error":"task 99 not found"}
```

No part of this path needs special error-handling code in the handler, since the types carry the failure through to the response.

## 10. Tests: `tests/api.rs`

### The helper function

```rust
fn test_app() -> Router {
    routes::app(TaskStore::new(), Duration::from_secs(5))
}
```

Each test creates its own store, so tests can't affect each other even though Cargo runs them in parallel.

The `send` helper keeps the tests short. It builds a request, sends it, and returns the status and the parsed JSON:

```rust
    let response = app.clone().oneshot(request).await.unwrap();
```

A `Router` is also a Tower `Service`, an object that takes a request and returns a response. `oneshot`, from `tower::ServiceExt`, sends one request directly to that service. No network port is opened, so the tests are fast and can't conflict with other programs. The method consumes the router, so the helper clones it first. The clone shares the same store, so a test can send several requests that build on each other.

```rust
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
```

A response body arrives as a stream. `collect`, from `http_body_util::BodyExt`, reads it to the end so it can be parsed.

### A typical test

```rust
#[tokio::test]
async fn health_reports_ok() {
    let (status, body) = send(&test_app(), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "status": "ok" }));
}
```

`#[tokio::test]` is the counterpart of `#[tokio::main]`: it provides a runtime so the test can be `async`. The tests compare complete JSON values, because the JSON format is what real clients depend on.

Strings such as `r#"{"title":"a"}"#` are raw strings. Inside them, quotes need no backslashes, which makes JSON easier to read.

### Testing the timeout

The real endpoints are all fast, so this test creates its own slow route:

```rust
    let app = routes::with_middleware(slow, Duration::from_millis(10));
```

The route sleeps for 5 seconds, but the test finishes after about 10 milliseconds with a `504`, because the middleware cancels the handler. This is why `with_middleware` is public and separate from `api_routes`.

## 11. Concept summary

| Concept | Where it's used | Purpose |
|---|---|---|
| `async` and `.await` | Handlers, `main` | Waiting without blocking a thread |
| `#[tokio::main]` | `main.rs` | Provides the async runtime |
| `tokio::select!` | `shutdown_signal` | Waits for the first of several events |
| `#[cfg(...)]` | `shutdown_signal` | Chooses platform-specific code at compile time |
| Generics and trait bounds | `read_var` | One function for all parsable types |
| `#[derive(...)]` | Most types | Generates standard functionality |
| Serde | `models.rs` | Converts between structs and JSON |
| `Option` | `UpdateTask`, store methods | A value that may be absent |
| `Result` and `?` | Everywhere | Passes errors up to the caller |
| `Arc` | `TaskStore` | Shared ownership across threads |
| `RwLock` | `TaskStore` | Safe changes to shared data |
| Guards (RAII) | Store methods | Releases the lock automatically |
| `thiserror` | `AppError` | Generates error messages and conversions |
| `IntoResponse` | `AppError`, handler return values | Converts values into HTTP responses |
| Extractors | Handler parameters | Get typed data from the request |
| Tower layers | `with_middleware` | Wrap handlers with shared behavior |
| `oneshot` | `tests/api.rs` | Sends requests without a network |
