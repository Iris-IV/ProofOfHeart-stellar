## Storage TTL Policy

To avoid rent inflation from read-heavy clients/indexers:

- Persistent storage TTL is extended on **writes only**.
- Read/view functions return stored values without extending TTL.

Write paths still call `extend_ttl` for touched keys, preserving liveness for actively updated entries while keeping read operations economically neutral.
