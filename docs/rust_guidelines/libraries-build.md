# Libraries / Building (progressive)

<agent>
<goal>Make libraries build reliably across platforms and feature sets.</goal>
<when_to_use>When designing Cargo features, publishing crates, or building `-sys` wrappers.</when_to_use>
<contains>M-FEATURES-ADDITIVE, M-OOBE, M-SYS-CRATES</contains>
<see_also>libraries-interop.md, libraries-resilience.md</see_also>
<canonical>../rust_guidelines_full.md</canonical>
</agent>

**TL;DR:** Features must be additive, libraries should build out-of-the-box on Tier 1 without extra deps, and `-sys` crates must govern their native builds to “just work”.

**Checklist:**
- Make features additive; any combination should compile without removing items.
- Avoid `no-std` feature flips; use `std` feature instead.
- Ensure crates build on Tier 1 with only Rust/Cargo present; gate platform specifics.
- `-sys` crates: own the native build in `build.rs`, avoid external tools, embed/verifiable sources, support static + dynamic linking.

## Features are Additive  (M-FEATURES-ADDITIVE) { #M-FEATURES-ADDITIVE }

<why>To prevent compilation breakage in large and complex projects.</why>
<version>1.0</version>

All library features must be additive and work in any combination (when platform-appropriate):
- No `no-std` feature; use a `std` feature instead.
- Adding a feature must not remove/alter public items (adding non-exhaustive enum variants is fine).
- Features should not require manual enabling of other features or parent/child coupling.

Further reading: [Feature Unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification), [Mutually Exclusive Features](https://doc.rust-lang.org/cargo/reference/features.html#mutually-exclusive-features).

## Libraries Work Out of the Box (M-OOBE) { #M-OOBE }

<why>To be easily adoptable by the Rust ecosystem.</why>
<version>1.0</version>

Libraries must build on all Tier 1 platforms without extra prerequisites beyond Rust/Cargo. If tools are needed (e.g., codegen), run them before publishing and ship generated artifacts. Gate platform-specific dependencies with cfg/feature flags. Libraries are responsible for their dependency chains; avoid imposing external tools on downstream consumers.

## Native `-sys` Crates Compile Without Dependencies (M-SYS-CRATES) { #M-SYS-CRATES }

<why>To have libraries that 'just work' on all platforms.</why>
<version>0.2</version>

For `foo`/`foo-sys` pairs wrapping native libs:
- Govern the native build in `build.rs` via [`cc`](https://crates.io/crates/cc); avoid external build scripts/Makefiles.
- Make external tools optional; embed upstream sources and include verifiable URL+hash.
- Pre-generate `bindgen` where possible.
- Support static linking and dynamic loading (e.g., via [`libloading`](https://crates.io/crates/libloading)).

### Related
- Interop: `libraries-interop.md`
- Resilience: `libraries-resilience.md`
- Original: `../rust_guidelines_full.md`
