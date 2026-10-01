This document is an append-only journal dumping my thoughts and intentions while 
developing this

# 2026-10-02 11:30

- Conjecture: dependency graph is a sparse matrix, so we might want to run Kahn
  over a compressed-sparse row.
  - Some tasks have O(n) dependencies, _e.g._ post indices
  - Most tasks have O(1), _e.g._ the posts themselves; there are more posts than
    there are indices

# 2026-10-02 11:00

- Starting to work on Task

# 2026-09-30 12:00

- We probably don't want to async this if we don't need to? Conjecture is that
  multithreading would make waiting for I/O cheap -- that thread will just sleep
  while others work, the OS can deal with it for us. We could bench anyways,
  though, but later.

# 2026-09-30 10:45

- We'll implmement an `Fs` trait abstracting filesystem behaviour, rather than
  interacting with `std::fs` directly, so that we can do things like fault
  injection for testing.
- [x] Implement an `Fs` trait for swappable filesystem abstractions.

# 2026-09-29 22:30

- A `Store` can be idealised as a `HashMap`. We can use that to implement
  differential testing; just continuously fuzz and feed the same random bytes to
  the two, and detect any behavioural differences.

# 2026-09-29 21:45

- When `Task` is implemented, we need to test that for all tasks, incremental
  build (_i.e._, with a warm cache) is always equivalent to a clean build
  (_i.e._, with a cold cache).

# 2026-09-29 20:00

- We'll implement the public functions of `Store`, some tests, and hook them
  up to the CLI before doing the actual impl
- Apparently `blake3` does not implement `std::error::Error` for its error types
  without the `std` feature enabled for it, so we'll enable that.

# 2026-09-29 18:00

- `Tasks` can be modelled as a table, with the singular `Task` being a
  projection -- a view, if you will -- containing references to the appropriate
  fields in the table
- A `Task` function can be faithful to the definition in Build Systems a la
  Carte -- _i.e.,_, it maps a store `S` and a key `k` to a store `S'`, where
  `S'` is `S` but with the value associated with `k` being ensured to be
  up-to-date.
- That is, the store (or a reference to it) becomes the input to a
  task, rather than some arbitrary object.
- We also want a CLI to mess around with `Store`
- [ ] Implement `Store` as an object storage before we can implement `Task`
  - [ ] Implement CLI to interact with `Store`

# 2026-09-29 16:00

- Architecture framed around Build Systems a la Carte, as a build system with
  topological scheduler, verifying trace rebuilder, and applicative tasks (no
  scripting)
- Deps are known statically so we can run a topological sort on the global
  set of tasks to determine run order.
  - Look into how to maintain this ordering for potentially parallel execution
- Red-green algo from rustc for caching.
  - Need a way to track the identity of tasks
    - Theoretical ideal is hashing something like the MIR of tasks
      - Probably do this after v1
    - Dumb solution would probably be `name:version` with a manually bumped
      `version`
  - SQLite to keep track of build traces -> `rusqlite`
- Content-addressable storage to store intermediary results hashed with `blake3`
  - blake3 has ~equivalent collision resistance to SHA-512 but is faster
  - We can consider compressing objects with e.g. zstd in the future, for now,
    we will just store it as-is
  - Maybe objects are stored as json with `serde-json` to make it easier to
    convert to/from rust structs? 
    - Or maybe `bincode`, since we don't strictly need it to be plaintext. 
    - As long as the representation strictly roundtrips correctly, we can use
      whatever codec is fastest
- [ ] Implement `Task` as a model as a pure function over some declared set of
      inputs to an output that is storable in the content-addressable storage.
      - All `Task` must have a uniform interface for feeding it its dependencies
        - We could just have `Task` interface with the object storage itself;
          scheduler then guarantees that all its dependencies are fresh by the
          time it is executed
