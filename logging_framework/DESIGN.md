# Logging framework — Design

A thread-safe, extensible logging framework. Mirrors the workspace's crate
layout (lib + bin, `domain/` for core types, `adapters/` for I/O).

## Requirements → implementation

| Requirement | Implementation |
|---|---|
| Log levels Debug / Info / Error | `LogLevel` enum. Also carries `Warn` (between Info and Error) for filtering. |
| Multiple destinations | `LogWriter` trait; `ConsoleWriter` and `FileWriter` implement it. New backends (e.g. a database sink) implement `LogWriter` without touching the logger. |
| Configurable level + destination | `LogConfig` holds the level (and a `prefix`). The destination is chosen at construction by passing a `LogWriter`. |
| Thread-safe concurrent logging | The writer is stored behind `Box<dyn LogWriter>` inside a mutex. `log` serializes the mutex around each `write`, so messages cannot interleave. |
| Extensible levels + destinations | Levels are an ordered enum used for filtering; destinations implement the `LogWriter` trait. |

## Module layout

- `domain/level.rs` — `LogLevel`. Ordering (Debug < Info < Warn < Error) drives
  the threshold filter (`message.level >= config.level`).
- `domain/message.rs` — `LogMessage` value object rendered by writers.
- `domain/config.rs` — `LogConfig` (level + prefix). Configuration only; no I/O.
- `domain/writer.rs` — `LogWriter` trait, the extension point.
- `domain/logger.rs` — `Logger`: filters, tags, then dispatches under the mutex.
- `adapters/writers.rs` — concrete destinations (`ConsoleWriter`, `FileWriter`)
  plus a `CapturingWriter` test double.
- `error.rs` — `LogError` / `LogResult`.

## Concurrency model

One `Box<dyn LogWriter>` shared via a `Mutex`. This is deliberately the simple,
correct design: it serializes writes (no torn interleaving) at the cost of a
single writer bottleneck. Threads share the `Logger` through `Arc<Logger>`. A
scalable alternative (sharded records per thread, async batching) is possible
later without touching the public `LogWriter` shape.

The `Mutex` only guards writing, never level filtering — filtering happens
before the lock is taken, so a filtered-out message never contends.

## Error handling

All fallible ops return `LogResult<T>` (`thiserror`). File open failures map to
`LogError::WriterInit`, write failures to `LogError::Write`, and a poisoned lock
to `LogError::Poisoned`. `main()` unwraps only at the top level, where panics
are acceptable.

## Build & run

```sh
cargo run    -p logging-framework                         # demo
cargo test   -p logging-framework
cargo clippy -p logging-framework --all-targets -- -D warnings
cargo fmt    -p logging-framework --check
```