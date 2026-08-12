# Car Rental System — Design

Rust implementation of the car-rental design problem, restructured on the
same architecture as `flight_management`. The interesting part is that the
concurrency/consistency requirements map directly onto ownership, `Arc`,
`RwLock`, atomics, and `Result`-based error handling: the design has to be
explicit about locking and error paths.

## Requirements → Rust decisions

| Requirement | Rust mechanism |
|---|---|
| Browse and reserve cars for specific dates | `CarSearch` scans the fleet filtered by criteria + an availability check over the reservation book |
| Search by type, price range, availability | `search(make, model, year, max_price, start, end)`; a car must be `Available` and have no overlapping `Pending`/`Confirmed` reservation |
| Create / modify / cancel reservations | `ReservationManager` with two-phase transactions (below) and an explicit status state machine |
| Track car availability, update status | `Vehicle::status` interior mutability; `Available → Loaned → Available` on start/complete |
| Customer info + driver's license | `Customer` entity registered via `register_customer` |
| Payment processing | `PaymentGateway` trait seam + `PaymentProcessor` with LIFO refunds; charge/refund never run under a lock |
| Handle concurrent reservations | `Arc<RwLock<T>>` interior mutability + atomic counters; the `Pending` hold makes double-booking impossible (proved by `test_concurrent_booking_race`) |
| Single instance | Composition root `CarRentalSystem::with_gateway` builds every component once, shared via `Arc` |

## Concurrency model

**Lock ordering rule.** No lock is ever held while another lock is acquired.
`book`/`modify`/`cancel` mutate the reservation book under one write lock,
drop it, then talk to the payment gateway; `start_rental`/`complete_rental`
mutate the book, drop it, then mutate the vehicle status. `remove_vehicle`
reads the book, drops it, then removes from the registry. This gives a total
order with no deadlocks.

**Double-booking is impossible.** `book` inserts a `Pending` reservation
under the book's write lock before the payment call. Two threads booking the
same car serialize on that lock; exactly one sees no conflict. Proven by
`test_concurrent_booking_race` (16 threads, one car → exactly one winner).

**No I/O under a lock.** Gateway calls happen outside the book lock. The
`Pending` hold is short-lived; payment latency does not block other bookings,
and a payment failure rolls the reservation back to `Cancelled` (audit
trail).

**Poisoned locks.** `locks::rd`/`wr` recover the value from a poisoned lock
(`PoisonError::into_inner`) instead of panicking; a panic in one thread
should not take the whole service down.

## Reservation lifecycle

`book`: validate dates/vehicle, write `Pending` under the book lock, charge
outside the lock, re-commit as `Confirmed`; on charge failure the reservation
is left as `Cancelled` for the audit trail and the error is returned.

`modify`: validate ownership and state, check the new window against *other*
reservations (self is excluded — moving your own reservation over its old
dates is legitimate), write the new range as `Pending` so the car is held
while the price delta is settled, settle outside the lock, re-commit as
`Confirmed`. On settlement failure the original reservation is restored.

`cancel`: mark `Cancelled` under the book lock (the car is immediately
bookable again), then refund outside the lock. A refund failure returns an
error but never resurrects the reservation.

`start_rental` / `complete_rental`: `Confirmed → Active` / `Active →
Completed`, with the vehicle `Available → Loaned → Available`.

## Layering

```
adapters/  MockGateway                     — crosses the PSP boundary
domain/    accounts, payments, reservations, search, system, vehicles
           (core types + `PaymentGateway` trait; no I/O)
```

Rules:

- Domain has no provider dependencies; `CarRentalSystem::with_gateway` is the
  composition root that wires everything. A real PSP is an adapter swap, not
  a redesign.
- Naming: `CarError` / `CarResult<T>` per the error-naming convention; each
  enum lives with the domain it describes.
- Errors use `thiserror` and propagate with `?`; `unwrap`/`expect` appear
  only in tests and the demo binary.
- No pass-through wrappers: `CarRentalSystem` does not re-expose
  `book`/`cancel`/`search`; callers use the public component fields directly.

## Trade-offs (known and deliberate)

- **No persistence.** Everything lives in memory behind locks. A real system
  swaps `VehicleRegistry`/`ReservationBook` internals for a database; the
  domain types and the transaction shape carry over unchanged.
- **Refunds are at-least-once, not exactly-once.** A gateway failure after
  the provider already applied the refund would double-refund on retry; a
  production system needs idempotency keys. The LIFO refund returns the
  amount actually applied so the caller can reconcile.
- **`RwLock` over `HashMap`** is fine for a single process and demo scale;
  beyond one node it must become a distributed lock or a transactional store.
- **Cancellation has a race window** between "mark Cancelled" and "refund":
  the seat is bookable before the refund completes. That is safe (no
  double-booking) but is a UX latency, not a consistency issue.
- **Dates are exclusive-range.** `end_date` is the first day the car is free
  again, so `nights = end - start` and adjacent bookings never collide —
  unlike the original single-file version, which used inclusive dates and a
  `+1` correction.
