# Digital Wallet — Design Decisions

Mirrors the structure of the flight/car systems in this workspace: a lib + bin
crate with a `domain/` tree and a composition root, integer money, `thiserror`
for errors, and `Arc<RwLock<_>>` + `AtomicU64` counters for concurrency.

## Requirements → implementation

Requirement numbers below are the numbered list in `readme.md`, which is the
spec of record; this document only records decisions taken on top of it.

| Requirement | Where |
|---|---|
| 1. users & personal info | `domain/accounts.rs` (`User`) |
| 2. add/remove payment methods | `domain/payments.rs` enums; `DigitalWallet::add/remove_payment_method` |
| 3. transfers (to external accounts too) | `domain/transactions.rs`; funding links a payment method |
| 4. history & statement | `TransactionManager::history_for`, derived from the log |
| 5. multiple currencies + conversion | `money.rs` `Currency` + `CurrencyConverter` |
| 6. security | tokenized card, KDF placeholder, no PAN/plaintext password, method-ownership check, non-enumerable auth |
| 7. concurrent transactions, consistency | single write-lock transfer engine + atomic counter ids |
| 8. scalability | lock-serialized point for correctness; see note below |

## Money is integer

Every balance is a count of **minor units** (`i64`) per currency. `f64` is never
used for money, so floating point cannot round-triple a cents balance.

A `Money` is a minor-unit count **plus** its `Currency`, and the two are never
separated. `Account::is_denominated` gates every balance change, so an amount
tagged INR can never be debited from a USD balance as if it were cents. Without that check the pair
(deposit 10000 INR, transfer 8300 INR) applied to USD accounts silently
destroyed value; the guard is what makes the conservation invariant hold for
*any* input rather than only well-formed ones.

## Cross-currency conversion: pivot-through-USD

Rates are quoted once per USD (`Currency::usd_minor_rate`) and any pair
converts by pivoting through USD. The rate table is an exhaustive `match`, not
a lookup array, so adding a currency without a rate is a compile error rather
than a runtime one — which is also why the function is infallible and no
"unsupported currency" error variant exists. Interior math uses `i128` so the
`amount × to_rate ÷ from_rate` product cannot overflow `i64`; a result that
still will not fit is an explicit `ConversionOverflow`. Conversion happens
**inside the single transfer write lock**, so the rate a transfer is priced at
can never race with the balance mutation following it. Our demo rates hang the
demo balances around "€100 ≈ $111".

## The transfer is one atomic step

`TransactionManager` owns the account ledger outright (`RwLock<HashMap<account,
Account>>`); `DigitalWallet` exposes it as the public `tx` field and adds no
pass-through wrappers, so there is one owner of balances and one way to reach
them. `transfer` takes the single **write** lock, grabs both accounts at once
with `HashMap::get_disjoint_mut`, and then separates deciding from committing:

- `Account::balance_after_debit` / `_after_credit` return what each balance
  *would* become, validating denomination, sufficiency and overflow, without
  touching anything.
- Only once both succeed are the two balances assigned. Those assignments
  cannot fail.

So no error path — and no panic, including a debug overflow panic — can leave
one leg applied and the other not, and no `expect` is needed to re-find
accounts that were already checked. Because there is exactly one account lock
there is no lock-ordering/deadlock problem, and the history append happens
*under* that same lock, so a committed balance change is visible in the
statement the instant it is visible in the balance, and log order is commit
order (no sorting needed on read).

## No pass-through wrappers

`DigitalWallet` used to re-expose `transfer`, `account` and `statement` as
one-line forwards to `tx`. Callers now use `wallet.tx.transfer(..)`,
`wallet.tx.account(..)` and `wallet.tx.history_for(..)` directly. What stays on
the wallet is what owns logic of its own: `fund_account` (method ownership),
`open_account` (currency uniqueness), `authenticate`, and the user/method
directories.

## The statement is derived, not stored

An `Account` keeps no list of transaction ids. That list would duplicate the
history log and give two structures to keep in step; instead `history_for`
filters the log for movements touching the account. This also removes the four
extra map lookups a transfer needed to link its id to both accounts, and drops
statement lookup from O(history × account-txs) to O(history).

## Both legs of a transfer are recorded

`Transaction::amount` is always what left the source, in the source's currency;
`TransactionKind::Transfer { credited }` carries what the destination received.
Storing only the credited amount made the sender's own statement report a
figure in a currency the sender never held.

This is the "synchronization + consistency" of requirement 7. It serializes all
transfers on one lock — correct and simple. For scale (requirement 8) the
improvement path is the classic per-account-sequencer / sharded-ledger model,
where each account has its own lock and transfers acquire two locks in a fixed
(u64-ordered) order. That removes the global contention point without weakening
atomicity.

## Payment methods: enum, not class hierarchy

The spec sketches an abstract `PaymentMethod` base with `CreditCard` /
`BankAccount` subclasses. For a closed set an exhaustive `enum` is simpler and
statically more honest: `match` must handle all variants, there is no dynamic
dispatch, and it is clippy-friendly. A name-conflicting trait pair would only be
needed if callers should be able to plug any third-party processor via a trait
object.

## Security notes (requirement 6)

- **No PAN/CVV/expiry retained.** `CardDetails` keeps only `last_four` (for
  display) and a `processor_token` that the real PSP would charge; the full card
  number never enters the model.
- **Passwords are a `hash(...)` placeholder.** A production build stores an
  Argon2/BCrypt hash and verifies with constant-time comparison and a salt. The
  placeholder keeps the data model honest without shipping a KDF dependency.
- Bank `routing_code` similarly only names the network target.
- **Payment methods are owned.** `add_payment_method` records the attaching
  user, and `fund_account` refuses a method whose owner is not the account
  holder — otherwise knowing a method id would be enough to charge someone
  else's card.
- **Authentication does not leak.** An unknown email and a wrong password both
  return `AuthFailed`, so the endpoint cannot be used to enumerate registered
  addresses.

## Concurrency tests

`transactions.rs` and `wallet.rs` tests spawn many threads pushing concurrent
transfers and assert that (a) no overdraft happens, (b) the summed final
balance equals the initial balance — the closed-system conservation invariant
that requirement 7 guarantees — and (c) the log holds exactly the committed
transfers, in increasing id order. `wallet.rs` additionally races eight threads
to open the same currency for one user and asserts exactly one wins.

Test names follow the workspace rule of at most three words after `test_`.

## Build & run

```sh
cargo run    -p digital-wallet                 # demo
cargo test   -p digital-wallet                 # unit + concurrency
cargo clippy -p digital-wallet -- -D warnings  # lint gate
cargo fmt    -p digital-wallet --check         # formatting gate
```