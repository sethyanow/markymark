## Essential Crates — Ecosystem Guide

> **TL;DR:** Don't reinvent the wheel. Rust's ecosystem has mature crates for most common
> tasks. This table maps use cases to recommended crates.

### Crate Selection by Use Case

| Use Case | Recommended Crate(s) | Notes |
|----------|----------------------|-------|
| **Serialization** | `serde` + `serde_json`, `serde_yaml`, `toml` | Derive-based, zero-boilerplate |
| **HTTP Client** | `reqwest` | Async by default, built on hyper |
| **HTTP Server** | `axum`, `actix-web` | axum: tower-based; actix: actor model |
| **Async Runtime** | `tokio` | De facto standard |
| **CLI Parsing** | `clap` (derive) | Proc macro for arg parsing |
| **Logging** | `tracing` + `tracing-subscriber` | Structured, spans, async-aware |
| **Error Handling (lib)** | `thiserror` | Derive Error for library types |
| **Error Handling (app)** | `anyhow` or `eyre` | Contextual app errors |
| **Database** | `sqlx`, `diesel`, `sea-orm` | sqlx: async, compile-checked SQL |
| **Testing** | `proptest`, `rstest`, `mockall` | Property, parameterized, mocking |
| **Benchmarking** | `criterion`, `divan` | Statistical benchmarks |
| **Date/Time** | `chrono`, `time` | time: lighter; chrono: more features |
| **UUID** | `uuid` | v4, v7, serde support |
| **Regex** | `regex` | Fast, safe, no backtracking |
| **Random** | `rand` | Trait-based, multiple algorithms |
| **Configuration** | `config` | Layered config from files, env, args |
| **Parallel Iteration** | `rayon` | Data parallelism with par_iter |
| **Channels** | `crossbeam-channel` | Multi-producer, multi-consumer |
| **TLS** | `rustls` | Pure Rust TLS, no OpenSSL |
| **Hashing** | `ahash`, `xxhash-rust` | Fast non-crypto hashing |
| **Crypto** | `ring`, `rustcrypto` | ring: audited; rustcrypto: modular |
| **FFI Bindings** | `bindgen`, `cbindgen` | C→Rust, Rust→C header gen |
| **Build System** | `cc` | Compile C/C++ in build.rs |
| **Allocator** | `mimalloc` | Drop-in perf improvement |
| **Compression** | `flate2`, `zstd` | gzip, zstd |
| **WASM** | `wasm-bindgen`, `wasm-pack` | Rust→WebAssembly |

### Crate Evaluation Criteria

Before adding a dependency, check:

1. **Maintenance**: Recent commits, responsive maintainers
2. **Downloads**: High download count on crates.io
3. **Dependencies**: Minimal transitive deps
4. **MSRV**: Compatible with your minimum supported Rust version
5. **License**: Compatible with your project
6. **Safety**: Minimize `unsafe` in dependencies — use `cargo-geiger`

### References

- crates.io: [crates.io](https://crates.io/)
- lib.rs: [lib.rs](https://lib.rs/) (curated categories)
- Guidelines: [M-OOBE](../../docs/rust_guidelines/libraries-build.md)
