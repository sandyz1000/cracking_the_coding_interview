# Airline Management System — Design

Rust implementation of a 1-hour design interview problem. The interesting part
is that the concurrency/consistency requirements map directly onto ownership,
`Arc`, `RwLock`, atomics, and `Result`-based error handling: there is no GC
and no exceptions, so the design has to be explicit about locking and error
paths.

## Requirements → Rust decisions

| Requirement | Rust mechanism |
|---|---|
| Search by (source, destination, date) | `FlightSearch` keeps a read-optimised `HashMap<(String, String, Date), Vec<String>>` index; rebuilt on schedule changes |
| Book / select seat / pay | two-phase transaction in `BookingManager::book`: **reserve → pay → commit**, with rollback on payment failure |
| Schedules, aircraft, crew | `Flight` holds `Arc<Aircraft>` + `RwLock<Vec<CrewMember>>`; `swap_aircraft` refuses swaps that orphan occupied seats |
| Passenger info + baggage | `Passenger` entity; `Baggage` attached per booking with an ownership check |
| User types | `UserRole::{Passenger, Staff, Admin}`; cancellation is owner-or-staff/admin, scheduling and crew need admin |
| Cancellations, refunds, changes | explicit status state machines; `cancel` = mark → release seat → refund; `change_flight` reserves the new seat first |
| Data consistency, concurrent access | `Arc<RwLock<T>>` interior mutability + atomic counters; lock ordering rules (below) |
| Scalable and extensible | `PaymentGateway` trait seam; seat layout derived from `Aircraft`; enums instead of strings; `AmsResult` everywhere |

## Concurrency model

**Lock ordering rule.** Only one nested lock acquisition is allowed anywhere
in the codebase: registry read → seat write (`BookingManager::release_seat_for`).
Everything else acquires locks sequentially — the guard is dropped before the
next lock is taken. Scheduling drops the registry write guard before
rebuilding the search index. Searches take the index read guard, drop it,
then take the registry read guard. This gives a total order with no
deadlocks.

**Double-booking is impossible.** `Flight::reserve` performs check-and-set
(available → reserved) under the seat map's write lock. Two threads trying
the same seat serialize on that lock; exactly one sees `Available`. This is
proven by `test_seat_race` (64 threads, one seat → exactly one winner) and
`test_full_inventory` (36 threads, all distinct seats → all succeed).

**No I/O under a lock.** Gateway calls (charge/refund) happen outside the
inventory lock. The reservation is short-lived; payment latency does not
block other passengers, and the rollback on payment failure releases the
seat.

**Poisoned locks.** `locks::rd`/`wr` recover the value from a poisoned lock
(`PoisonError::into_inner`) instead of panicking; a panic in one thread
should not take the whole service down.

## Booking lifecycle

`book`: reserve (write lock, check-and-set) → charge outside the lock →
confirm; on charge failure, release and return `AmsError::PaymentFailed`.

`cancel`: validate ownership and state under the bookings write lock, mark
`Cancelled`, release the seat, then refund. Booking state is committed before
the refund so a refund failure cannot leave a seat locked. The refund is
LIFO and returns the amount actually applied.

`change_flight`: the new seat is reserved **first**, so the passenger is
never left seatless. The fare difference is settled (extra charge, or
partial refund) and the reservation rolled back if settlement fails. Then
the new seat is confirmed, the old seat released, and the booking updated
with its running refund total.

## Layering

```
adapters/  MockGateway                 — crosses the PSP boundary
domain/    accounts, bookings, flights, payments, search, system
           (core types + `PaymentGateway` trait; no I/O)
```

Rules:

- Domain has no provider dependencies; `AirlineManagementSystem::with_gateway`
  is the composition root that wires everything. A real PSP is an adapter
  swap, not a redesign.
- Naming: `AmsError` / `AmsResult<T>` per the error-naming convention; each
  enum lives with the domain it describes; `FlightSpec` bundles the
  schedule fields so `schedule_flight` stays at five arguments.
- Errors use `thiserror` and propagate with `?`; `unwrap`/`expect` appear
  only in tests and the demo binary.
- No pass-through wrappers: `AirlineManagementSystem` does not re-expose
  `book`/`cancel`/`search`; callers use the public component fields directly.

## The "Singleton" requirement

The problem statement asks for singleton `BookingManager` / `PaymentProcessor`.
In Rust the idiomatic answer is a composition root: build each component
once in `AirlineManagementSystem::with_gateway` and share it through `Arc`.
This preserves the property the pattern exists for (one coordinated
instance) without process-global mutable state, which would make tests and
swap-in adapters painful. When a true process-wide global is genuinely
required, `std::sync::OnceLock` is the correct primitive (used in the
original prototype as `PaymentProcessor::global`).

## Trade-offs (known and deliberate)

- **No persistence.** Everything lives in memory behind locks. A real system
  swaps `FlightRegistry`/`BookingManager` internals for a database; the
  domain types and the transaction shape (reserve → pay → commit) carry over
  unchanged.
- **Refunds are at-least-once, not exactly-once.** A gateway failure after
  the provider already applied the refund would double-refund on retry; a
  production system needs idempotency keys. The LIFO refund returns the
  amount actually applied so the caller can reconcile.
- **`RwLock` over `HashMap`** is fine for a single process and a demo scale;
  beyond one node it must become a distributed lock or a transactional store.
- **Cancellation has a race window** between "mark Cancelled" and "release
  seat"; a concurrent booker can observe the seat as still occupied. That is
  safe (no double-booking) but is a UX latency, not a consistency issue.
- **`Flight::reserve`/`confirm`/`release` are `pub(crate)`** state machines;
  they document the seat lifecycle and keep the invariant that only the
  booking manager transitions seat state.
