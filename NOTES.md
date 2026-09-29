This document is an append-only journal dumping my thoughts and intentions while 
developing this

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
