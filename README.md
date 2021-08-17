# Surf-pool

Surf-pool is a crate that allows to reuse existing `http` connections.

This connection pool can be useful especially with `https`, because it would avoid to perform the handshake every time.

The crate is based on `async-std` and `surf`
