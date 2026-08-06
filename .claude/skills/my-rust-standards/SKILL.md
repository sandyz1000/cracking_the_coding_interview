---
name: my-rust-standards
description: >
  Project-specific Rust engineering standards for the My Rust project.
  Use this skill when:
  (1) writing or refactoring any Rust code in this workspace,
  (2) reviewing a PR or diff for style, structure, or naming issues,
  (3) adding new modules, crates, or public API surface,
  (4) writing or renaming test functions,
  (5) deciding whether a helper function or wrapper is justified.
license: MIT
compatibility: Rust stable, Cargo workspace
metadata:
  author: my
  version: "1.0.0"
allowed-tools: Bash(cargo:*) Bash(rustfmt:*) Bash(clippy:*) Read Write Edit Glob Grep
---

# My Rust Engineering Standards

Apply these rules whenever writing, reviewing, or refactoring Rust code in this workspace. They
extend the project's CLAUDE.md and take precedence over general Rust style where they conflict.

---

## Role

Act as a **senior Rust engineer**. The goal is idiomatic, readable, maintainable code — not
clever code. When in doubt, choose the boring, obvious solution.

---

## Code Style

### Formatting and linting

- All code must pass `rustfmt` without manual overrides.
- All code must pass `cargo clippy -- -D warnings`. Fix the root cause; do not suppress with
  `#[allow(...)]` unless the lint is genuinely wrong for this case (add a comment explaining why).
- Prefer `#[expect(clippy::lint)]` over `#[allow(...)]` — it fails if the lint disappears.

### Error handling

- Return `Result<T, E>` for all fallible operations. Use `?` to propagate.
- Never use `unwrap()` or `expect()` in production code paths. Reserve them for tests and
  `main()` where a panic is acceptable.
- Define errors with `thiserror` for library crates. Use the pattern `FooError` / `FooResult<T>`.
- Do not swallow errors with `let _ = ...` unless the discard is intentional and obvious.

### Ownership and borrowing

- Prefer `&T` over `.clone()` when ownership is not needed.
- Use `&str` / `&[T]` in function parameters; return `String` / `Vec<T>` when ownership is given.
- Small `Copy` types (≤ 32 bytes) may be passed by value.

---

## Project Structure

### Group by responsibility, not by kind

Modules must reflect a domain concept or adapter boundary, not a category of language construct.

```
// Bad
utils.rs
helpers.rs
types.rs

// Good
circuit_breaker.rs
shuffle_fetcher.rs
partition_cache.rs
```

### Accepted top-level groupings

| Group | What belongs there |
|-------|--------------------|
| `domain/` | Core types and traits with no I/O or framework deps |
| `adapters/` | Implementations that cross a system boundary (TCP, HTTP, FS) |
| `config/` | `Config` struct, env-var parsing, builder |

### Module files

- Every module must justify its existence. A file that only re-exports from one other file is
  indirection with no value — merge or delete it.
- `mod.rs` is acceptable for directories with multiple submodules. Single-file modules use
  `module_name.rs` directly.

---

## Naming

### Files and modules

- `snake_case` only.
- Name by responsibility, not by the Rust construct inside:
  `rdd_executor.rs` not `executor_impl.rs`, `shuffle_cache.rs` not `cache.rs`.

### Types and functions

| Kind | Convention | Example |
|------|-----------|---------|
| Error type | `FooError` | `SchedulerError`, `ShuffleError` |
| Result alias | `FooResult<T>` | `SchedulerResult<T>` |
| Entry-point function | verb + noun | `run_synthesis_job`, `dispatch_pipeline` |
| Builder method | `with_foo` | `with_driver_fingerprint`, `with_speculation` |
| Predicate | `is_` / `has_` | `is_cached`, `has_workers` |
| Conversion | `into_` / `as_` | `into_rdd`, `as_bytes` |

---

## Functions

### Each function must do one thing

A function whose entire body is a single delegating call to another function adds indirection
without value. Remove it and let callers call the target directly, or make the field `pub`.

```rust
// Bad — pure pass-through
fn get_context(&self) -> Arc<Context> { self.context.clone() }

// Good — make the field pub, or inline at call sites
pub context: Arc<Context>
```

Exceptions: trait implementations that must exist by contract (e.g. `impl Iterator`), and
builder methods that enforce an invariant beyond the assignment.

### No shallow wrappers in dispatch paths

In hot paths (task dispatch, shuffle write, pipeline execute), every function call must be
justified by logic it owns — routing, decoding, error translation, etc. A function that only
calls `inner.execute(...)` and returns the result should be inlined.

---

## Tests

### Naming — maximum 3 words after `test_`

Test names must be short and specific. 3 words after the `test_` prefix is the limit.

```rust
// Bad — too many words
fn test_partition_store_type_mismatch_returns_none_silently()
fn test_cache_prevents_recomputation_of_partitions()
fn test_register_map_output_without_register_shuffle_panics()

// Good
fn test_type_mismatch_silent()
fn test_no_recompute()
fn test_unregistered_shuffle()
```

Name format: `test_<subject>_<condition>` or `test_<subject>_<result>`. Both subject and
condition/result should be single words or well-known abbreviations (`oob`, `tcp`, `rdd`).

### Structure

- One logical assertion per test. Use multiple `assert_*` calls only when they verify the same
  invariant from different angles.
- Do not test implementation details — test observable outputs.
- `#[should_panic]` tests must include an `expected = "…"` substring to avoid masking unrelated panics.

### Placement

- Unit tests go in a `#[cfg(test)] mod tests { … }` block at the bottom of the file they test.
- Integration tests that span multiple crates go in `crates/my-tests/`.
- Tests that require a live worker process must be `#[ignore]` and documented with a one-line
  setup comment.

---

## Comments

Keep the codebase light. Comments are a last resort, not documentation. Extended rationale,
architecture, design decisions, and pipeline narrative belong in **docs** (a design doc, `CLAUDE.md`,
or a module-level `//!` one-liner that *points* to the doc) — not inline and not in multi-paragraph
doc-comments. A reader should learn *what* from the code and *why-it-exists* from the docs.

- At most a **single-line** `//` *why* on a genuinely non-obvious line.
- Public-item doc-comments (`///`) stay terse — one or two lines. If you need a paragraph to explain
  a design, write it in the doc and reference it, e.g. `/// See backends/DESIGN-x.md`.
- No multi-paragraph `///` blocks restating an algorithm or justifying an architecture.
- Delete a comment before it goes stale; prose drifts from code faster than code drifts from itself.

Only write a comment when the **why** is non-obvious. Never write a comment that restates what
the code already says.

```rust
// Bad — restates the code
// Append empty embedding column
let col = append_empty_embed_col(&batch);

// Bad — narrates a step number
// 1. Decode input
let items = decode(data)?;

// Good — explains a hidden invariant
// FxHasher is deterministic across processes (no random seed), which is required for
// consistent bucket assignment between driver and worker.
let mut hasher = FxHasher::default();
```

Section dividers (`// ── Foo ──────────`) are forbidden. Use blank lines and module structure
to group code instead.

---
