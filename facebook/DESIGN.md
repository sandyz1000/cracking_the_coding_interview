# Facebook-like Social Network — Design

Mirrors the reference systems in this workspace (flight/car): a lib + bin crate
with a `domain/` tree, `thiserror` errors, poison-recovering `rd`/`wr` lock
helpers, `AtomicU64` id counters, and `RwLock<HashMap<…>>` registries.

## Requirements → implementation

Requirement numbers below are the numbered list in `readme.md`, which is the
spec of record; this document only records decisions taken on top of it.

| # | Requirement | Where |
|---|---|---|
| 1 | profile, work, education | `Member.profile`; `add_work`/`add_education` |
| 2 | search members/groups/pages by name | `SearchIndex` word index |
| 3 | friend request send/accept/reject | `ConnectionInvitation` + `send_/accept_/reject_connection` |
| 4 | follow members without friendship | `follow_member`/`unfollow_member` |
| 5 | create/join groups, create/follow pages | `create_group`, `join_group`, `create_page`, `follow_page` |
| 6 | posts shared with friends | `create_post`, `like_post`, `share_post(sharer, post)` |
| 7 | comments, likes, shares, recommendation | `add_comment`, `like_comment`, `share_post`, `recommend_page` |
| 8 | privacy lists restrict post visibility | `PrivacyList` + `share_post_to_list`, enforced by `can_view`/`visible_posts` |
| 9 | direct messages | `send_message`, `messages_to` |
| 10 | page recommendations | `recommend_page` |
| 11 | notifications | `Notification` registry + `notifications_for` |
| 12 | search posts by word | `SearchIndex` post word index |
| ext | connection suggestions | `connection_suggestions` |

## The id/single-writer model

Every entity has its own id type (`MemberId`, `PostId`, … in `src/id.rs`), each a
newtype over `u64`, and every registry is keyed by its own. Passing a `PostId`
where a `MemberId` belongs is a compile error rather than a lookup that quietly
misses. Because the types keep them apart, each registry counts its own ids from
1 — ids are dense per entity, matching the spec's per-entity id naming
(`__member_id`, `__post_id`, …).

The counter is not a loose field beside the map: `Registry<Id, T>` owns both, and
`insert_new` is the only way to allocate. An id can therefore only be drawn for
the registry it is inserted into — drawing a `PostId` from the member registry is
a compile error, not a silent gap in the id sequence. `insert_new` is the whole
API; reads keep going through `rd(&reg.items)` rather than a wrapper method per
`HashMap` operation.

An earlier version drew every id from one shared `AtomicU64`. That made ids
disjoint across registries, so a mixed-up id merely failed to resolve — but the
protection was invisible, unenforced, and would have vanished silently the moment
anyone added a per-registry counter or persisted ids.

All registries are keyed by a typed id backed by a `u64`. Most read paths take a
read guard and clone; write paths take the write guard, mutate in place, and
drop it before returning. There is **no lock held while acquiring another**
within a single method, so no lock-ordering cycle — every method serialises on
the registry it touches and nothing else.

Operations that touch two registries (`follow_page`, `join_group`,
`recommend_page`, `add_comment`) scope the first guard in a block that ends
before the second is taken, so the "no nesting" rule holds literally rather than
by luck of drop order.

## Connection invitations

An invitation records `(from, to, status)`. `accept_connection(to, invite_id)`
verifies the recipient is `to` and the request is still `Pending`, then installs
the two-way edge via `link` (idempotent) and marks it `Accepted`. Because a
thread accepts by mutating the request under one write guard, the accept-vs-
double-accept race cannot double-install the edge.

Friend-graph reads such as `connection_suggestions` take a short-lived read
guard, clone what they need, and drop it — they never hold a read lock across a
write to the same registry (the classic self-deadlock this design avoids).

## Notifications (requirement 11)

A `notifications` registry is filed per recipient and fed by the three triggers
the spec names: `send_message`, `send_connection_invitation`, and `add_comment`
(skipped when a member comments on their own post). `NotificationKind` carries
the other party, so a reader never has to join back to another registry to
render a line. Delivery — push, email — is a transport concern and stays out of
the domain; `notifications_for` is the queue an adapter would drain.

## Privacy lists and post visibility

A privacy list is a named set of the owner's **friends** — `add_to_privacy_list`
rejects a member who is not connected to the owner, which is what makes the list
a subset of the audience rather than an arbitrary id set.

`can_view(viewer, post)` is the rule the requirement actually asks for:

- the author always sees their own post;
- a post with no linked list reaches the author's connections (requirement 6's
  "share with their friends");
- a post linked to one or more lists reaches only members of those lists.

`visible_posts(viewer)` is that predicate over the whole post registry, newest
first. Linking a post to a list is owner-only on both the list and the post, so
a third party cannot widen or narrow someone else's audience.

## Search vs. the spec's `SearchIndex`

The spec sketches a Python `Search` ABC with a `SearchIndex` implementation. Rust's
equivalent is a single concrete `SearchIndex` with a per-entity `add_*`/`search_*`
pair — no dynamic dispatch needed for one implementation. It stores an inverted
word index (`word -> [id]`), supporting prefix matching for member/group/page
names (e.g. `"Al"` → `Alice`) and whole-word post search. Updates happen under a
short write guard; reads under a short read guard.

## Blocking is a status, not a deletion

`block_member` sets the member's account to `Blacklisted`; it does not remove
the member row. Deleting it would orphan every post, comment, group membership
and inbound connection that referenced the id, and would force `unblock` to
take a caller-held stale copy of the member to resurrect. `require_active`
gates every member-initiated mutation on the account status, so a blocked
member simply cannot act, while their existing content stays readable and their
id keeps resolving.

## Connection suggestions (extended requirement)

`connection_suggestions(member)` computes a two-hop friendship BFS:

1. collect the member's direct connections
2. for each, walk *their* connections to find candidates two hops away
3. skip self and anyone already connected
4. rank by how many mutual friends each candidate shares (desc), tie-broken by
   id for determinism
5. cache the ranked list onto `Member.suggestions`

This is a small, deterministic graph computation with no I/O under lock.

## Build & run

```sh
cargo run    -p facebook                         # demo
cargo test   -p facebook                         # unit tests
cargo clippy -p facebook --all-targets -- -D warnings
cargo fmt    -p facebook --check
```