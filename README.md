# Surf-pool

Surf-pool is a crate that allows to reuse existing `https` connections.

This connection pool can be useful to reduce latency, because it would avoid to perform the handshake every time, but re-using a pre-existing and established connection.

The crate is based on `async-std` and `surf`
