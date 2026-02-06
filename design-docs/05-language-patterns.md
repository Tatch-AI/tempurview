---
marp: true
theme: ember
paginate: true
---

# Language Patterns

### Rust vs Go vs Python for the patterns behind TemPurview

---

## Why Compare?

TemPurview is built in Rust. Tempo (a similar tool) is built in Go.
Many Temporal users write workflows in Python.

Each language handles the same core patterns differently:

- **Enums & pattern matching** — how state machines are expressed
- **Error handling** — propagation, recovery, ergonomics
- **Traits vs interfaces** — polymorphism without inheritance
  - Three backends (gRPC, mock, CLI) all "look the same" to the rest of the code — the mechanism for achieving that differs by language
- **Async concurrency** — how concurrent work is expressed
- **Generics** — type-safe reuse
- **Ownership & sharing** — thread safety guarantees

The right language for a tool depends on which patterns it leans on hardest.

---

<!-- _class: compact -->

## Enums & Pattern Matching

TemPurview models **every possible state** as an enum variant.
The compiler enforces exhaustive handling.

**Rust** — algebraic data types with exhaustive matching:
```rust
pub enum LoadState<T> {
    NotLoaded, Loading, Loaded(T), Error(String),
}
match state {
    LoadState::Loaded(data) => render(data),
    LoadState::Error(msg) => show_error(msg),
    LoadState::Loading => show_spinner(),
    LoadState::NotLoaded => {},  // compiler forces this arm
}
```

**Go** — no sum types; use interfaces or constants:
```go
type LoadState int
const (NotLoaded LoadState = iota; Loading; Loaded; Error)
// No compiler enforcement — forgotten cases are silent bugs
```

**Python** — `Enum` class, no exhaustive matching:
```python
class LoadState(Enum):
    NOT_LOADED = auto(); LOADING = auto()
match state:  # 3.10+, no exhaustiveness check
    case LoadState.LOADING: show_spinner()
```

---

## Why This Matters for TemPurview

The `Action` enum has **120+ variants**. The `update()` function
matches on every single one. The compiler guarantees:

- Adding a new `Action` variant → **compile error** in every
  unhandled match, across the entire codebase
- No "default" arm silently swallowing new states
- `View`, `Effect`, `LoadState`, `WorkflowStatus` — all enums,
  all exhaustively matched

In Go or Python, a new state variant can be silently ignored.
In Rust, it's a build failure. For a tool managing production
workflows, that guarantee matters.

---

<!-- _class: compact -->

## Error Handling

TemPurview chains errors through `?` — every call site either handles or propagates.

**Rust** — `Result<T, E>` + `?` operator:
```rust
let channel = Endpoint::from_shared(addr)
    .map_err(|e| ClientError::ConnectionError(e.to_string()))?
    .tls_config(ClientTlsConfig::new().with_native_roots())
    .map_err(|e| ClientError::ConnectionError(e.to_string()))?
    .connect().await
    .map_err(|e| ClientError::ConnectionError(e.to_string()))?;
```

**Go** — explicit `if err != nil` at every step:
```go
endpoint, err := transport.NewEndpoint(addr)
if err != nil { return nil, fmt.Errorf("connection: %w", err) }
channel, err := endpoint.TLSConfig(tlsConfig)
if err != nil { return nil, fmt.Errorf("tls: %w", err) }
```

**Python** — exceptions (implicit propagation, easy to miss):
```python
try:
    channel = await connect(addr, tls=True)
except Exception as e:  # catches everything — too broad?
    raise ConnectionError(f"failed: {e}")
```

---

## Error Handling: The Tradeoff

| | Rust `?` | Go `if err` | Python `try/except` |
|--|----------|-------------|---------------------|
| **Propagation** | Automatic via `?` | Manual at every call | Automatic (implicit) |
| **Forgetting to handle** | Compile error | Silent — error ignored | Silent — exception flies |
| **Typed errors** | `enum ClientError` | `error` interface (untyped) | Exception hierarchy |
| **Context** | `.map_err()` chain | `fmt.Errorf("...: %w")` | Exception chaining |
| **Ergonomics** | Concise once learned | Verbose but readable | Concise but fragile |

TemPurview uses **6 error variants** in `ClientError`.
Every call site maps to the right one with `.map_err()`.
The type system prevents "connection error" being confused
with "parse error" — they're different enum variants.

---

<!-- _class: compact -->

## Trait-Based Polymorphism

TemPurview's `TemporalClient` trait enables 3 backends with zero runtime overhead.

**Rust** — traits (no inheritance, explicit impl):
```rust
#[async_trait]
pub trait TemporalClient: Send + Sync {
    async fn list(&self, filter: &WorkflowFilter, limit: u32)
        -> ClientResult<Vec<WorkflowSummary>>;
}
impl TemporalClient for GrpcTemporalClient { /* ... */ }
impl TemporalClient for MockTemporalClient { /* ... */ }
```

**Go** — interfaces (implicit satisfaction):
```go
type TemporalClient interface {
    List(ctx context.Context, filter WorkflowFilter) ([]WorkflowSummary, error)
}
```

**Python** — ABC or duck typing (no enforcement):
```python
class TemporalClient(ABC):
    @abstractmethod
    async def list(self, filter, limit): ...
```

---

## Trait Polymorphism: The Tradeoff

| | Rust traits | Go interfaces | Python ABC |
|--|-------------|---------------|-----------|
| **Satisfaction** | Explicit `impl Trait for T` | Implicit (structural) | Explicit or duck typing |
| **Dispatch** | Static or dynamic (`dyn`) | Always dynamic | Always dynamic |
| **Compile-time check** | Missing method → error | Missing method → error | Runtime `TypeError` |
| **Async support** | Via `async_trait` crate | Native (goroutines) | Native (`async def`) |
| **Multiple impls** | Unlimited, explicit | Unlimited, implicit | Unlimited, fragile |

Go's implicit interfaces are elegant for small interfaces.
Rust's explicit `impl` is safer for large trait surfaces
where accidentally satisfying an interface is a risk.

---

<!-- _class: compact -->

## Async Concurrency

TemPurview runs a TUI event loop, a CLI worker, and gRPC
calls concurrently. All coordinated through typed channels.

**Rust** — `tokio` runtime, `mpsc` channels, `select!`:
```rust
let (tx, mut rx) = mpsc::unbounded_channel::<Action>();
tokio::select! {
    Some(event) = events.next() => { /* handle input */ }
    Some(action) = rx.recv() => { /* handle async result */ }
}
```

**Go** — goroutines, channels, `select`:
```go
actions := make(chan Action)
select {
case event := <-events:  // handle input
case action := <-actions: // handle async result
}
```

**Python** — `asyncio`, no native channels:
```python
event_task = asyncio.create_task(get_event())
action_task = asyncio.create_task(rx.get())
done, _ = await asyncio.wait(
    [event_task, action_task], return_when=FIRST_COMPLETED)
```

Go and Rust are remarkably similar here.
Python's `asyncio` is more verbose and less intuitive for `select`-style multiplexing.

---

<!-- _class: compact -->

## Generics

`LoadState<T>` wraps any async data type with the same loading semantics.

**Rust** — monomorphized generics (zero-cost):
```rust
pub enum LoadState<T> {
    NotLoaded, Loading, Loaded(T), Error(String),
}
impl<T> LoadState<T> {
    pub fn as_ref(&self) -> Option<&T> {
        match self { LoadState::Loaded(t) => Some(t), _ => None }
    }
}
```

**Go** — generics (since 1.18):
```go
type LoadState[T any] struct {
    State  StateKind
    Data   T
    Error  string
}
```

**Python** — `Generic[T]` (type hints only, not enforced):
```python
class LoadState(Generic[T]):
    data: T | None = None
    error: str | None = None
```

---

<!-- _class: compact -->

## Ownership & Thread Safety

TemPurview shares a `TemporalClient` across threads using
`Arc<dyn TemporalClient>`. The compiler **proves** this is safe.

**Rust** — ownership + `Send`/`Sync` bounds:
```rust
// Arc = atomic reference count. dyn = dynamic dispatch.
// Send + Sync = compiler-verified thread safety.
let client: Arc<dyn TemporalClient> = Arc::new(grpc_client);
// Cloning Arc is cheap (increments a counter), not a deep copy
let worker_client = client.clone();
tokio::spawn(async move { worker_client.list(...).await });
```

**Go** — garbage collected, race detector at runtime:
```go
// Shared by default. Hope you don't mutate concurrently.
client := NewGrpcClient(addr)
go func() { client.List(ctx, filter) }()
// Data races caught at runtime, not compile time
```

**Python** — GIL (Global Interpreter Lock) protects, but limits parallelism:
```python
# The GIL is a mutex that lets only one thread execute Python
# bytecode at a time — "safe" by default, but single-threaded
client = GrpcClient(addr)
await asyncio.gather(client.list(f1), client.list(f2))
# True parallelism requires multiprocessing, not threading
```

---

## Builder Pattern & Method Chaining

TemPurview widgets use fluent builders — construct, configure,
render. The borrow checker ensures the builder outlives its data.

**Rust** — consuming `self` builder:
```rust
let widget = WorkflowListWidget::new(&workflows, &filter, &cols, &sort)
    .date_label(Some("Last 2h"));
// &workflows is borrowed — compiler proves it outlives the widget
```

**Go** — functional options pattern:
```go
widget := NewWorkflowList(workflows,
    WithFilter(filter),
    WithDateLabel("Last 2h"),
)
```

**Python** — keyword arguments (no builder needed):
```python
widget = WorkflowList(
    workflows=workflows, filter=filter, date_label="Last 2h"
)
```

Each language has its idiom. Rust's builder pattern
is more ceremony — but catches lifetime bugs at compile time.

---

## Summary: Why Rust for TemPurview

| Pattern | Rust Advantage |
|---------|---------------|
| Enums + matching | Exhaustive — compiler catches missing states |
| Error handling | `?` propagation + typed errors — no silent failures |
| Traits | Explicit impl + static dispatch — zero overhead |
| Async | `tokio` + typed channels — same ergonomics as Go |
| Generics | Monomorphized — zero runtime cost |
| Ownership | Compiler-proven thread safety — no data races |
| Single binary | No runtime, no GC, no interpreter — `cargo install` just works |

For a tool that manages **production workflows at scale**,
compile-time guarantees aren't academic — they're operational.

Every bug the compiler catches is one fewer silent failure
in your SRE toolchain.
