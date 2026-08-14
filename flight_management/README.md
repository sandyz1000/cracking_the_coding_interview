# Designing an Airline Management System

## Requirements

1. The airline management system should allow users to search for flights based on source, destination, and date.
2. Users should be able to book flights, select seats, and make payments.
3. The system should manage flight schedules, aircraft assignments, and crew assignments.
4. The system should handle passenger information, including personal details and baggage information.
5. The system should support different types of users, such as passengers, airline staff, and administrators.
6. The system should be able to handle cancellations, refunds, and flight changes.
7. The system should ensure data consistency and handle concurrent access to shared resources.
8. The system should be scalable and extensible to accommodate future enhancements and new features.

## Building and running

```sh
cargo run   -p flight-management-system                 # demo
cargo test  -p flight-management-system                 # unit + concurrency tests
cargo clippy -p flight-management-system -- -D warnings # lint gate
cargo fmt   -p flight-management-system --check         # formatting gate
```

Design rationale and concurrency model: see [DESIGN.md](DESIGN.md).

## Module map

```
src/
├── lib.rs          crate root + re-exports + test helpers
├── error.rs        AmsError (thiserror) / AmsResult<T>
├── locks.rs        poison-recovering RwLock accessors
├── time.rs         Date / Time value types
├── main.rs         demo binary
├── domain/         core types and the PaymentGateway trait, no I/O
│   ├── accounts.rs Passenger, User, UserRole
│   ├── bookings.rs Booking, Baggage, BookingManager (transactions)
│   ├── flights.rs  Aircraft, Flight, seat inventory
│   ├── payments.rs Payment, PaymentProcessor
│   ├── search.rs   FlightSearch + index
│   └── system.rs   AirlineManagementSystem (composition root)
└── adapters/       MockGateway — the PaymentGateway implementation
```

## Original class list

1. **Flight** — flight number, source, destination, departure time, arrival time, available seats.
2. **Aircraft** — tail number, model, total seats.
3. **Passenger** — ID, name, email, phone.
4. **Booking** — booking number, flight, passenger, seat, price, booking status.
5. **Seat** — seat number, seat type, seat status.
6. **Payment** — payment ID, method, amount, status.
7. **FlightSearch** — search by source, destination, date.
8. **BookingManager** — singleton-shaped; creation and cancellation of bookings.
9. **PaymentProcessor** — singleton-shaped; payment processing.
10. **AirlineManagementSystem** — main entry point combining all components.
