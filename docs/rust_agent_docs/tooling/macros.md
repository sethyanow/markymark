## Macros — Declarative & Procedural

> **TL;DR:** Use `macro_rules!` for simple pattern-based code generation, proc macros for
> derive/attribute macros. Prefer generics over macros when possible. Macros are powerful
> but harder to debug.

### macro_rules! vs Proc Macro vs Generics Decision Tree

```
Can you solve it with generics + traits?
├─ YES → Use generics (simpler, better errors, IDE support)
└─ NO
    Do you need to generate code from struct/enum shape?
    ├─ YES → Use derive proc macro (#[derive(MyTrait)])
    └─ NO
        Do you need compile-time code repetition/patterns?
        ├─ YES → Use macro_rules!
        └─ NO
            Do you need to transform arbitrary syntax?
            └─ YES → Use attribute proc macro (#[my_attr])
```

### macro_rules! Basics

```rust
// Simple macro
macro_rules! say_hello {
    () => { println!("Hello!") };
    ($name:expr) => { println!("Hello, {}!", $name) };
}

// Repetition macro
macro_rules! vec_of_strings {
    ($($item:expr),* $(,)?) => {
        vec![$($item.to_string()),*]
    };
}

let names = vec_of_strings!["Alice", "Bob", "Charlie"];
```

### Fragment Specifier Reference

| Specifier | Matches | Example |
|-----------|---------|---------|
| `$x:expr` | Any expression | `42`, `a + b`, `foo()` |
| `$x:ident` | Identifier | `my_var`, `String` |
| `$x:ty` | Type | `i32`, `Vec<String>` |
| `$x:pat` | Pattern | `Some(x)`, `_` |
| `$x:path` | Path | `std::io::Error` |
| `$x:stmt` | Statement | `let x = 5` |
| `$x:block` | Block | `{ println!("hi") }` |
| `$x:item` | Item (fn, struct, etc.) | `fn foo() {}` |
| `$x:literal` | Literal | `42`, `"hello"` |
| `$x:tt` | Single token tree | Any single token or `(...)` group |
| `$x:meta` | Attribute content | `derive(Debug)` |

### Repetition Syntax

```rust
// $(...),* — zero or more, comma-separated
// $(...),+ — one or more, comma-separated
// $(...);* — zero or more, semicolon-separated
// $(,)? — optional trailing comma

macro_rules! hash_map {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(map.insert($key, $value);)*
        map
    }};
}

let scores = hash_map! {
    "Alice" => 100,
    "Bob" => 95,
};
```

### Proc Macros (Overview)

Proc macros live in their own crate with `proc-macro = true`:

```toml
# Cargo.toml
[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"
```

```rust
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(MyTrait)]
pub fn my_trait_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl MyTrait for #name {
            fn describe(&self) -> String {
                format!("{} instance", stringify!(#name))
            }
        }
    };
    expanded.into()
}
```

### When NOT to Use Macros

- When generics work (better error messages, IDE support)
- For simple code that could be a function
- When the generated code is hard to debug
- When `impl Trait` or trait bounds solve the problem

### References

- The Rust Book: [Macros](https://doc.rust-lang.org/book/ch19-06-macros.html)
- Rust Reference: [Macros By Example](https://doc.rust-lang.org/reference/macros-by-example.html)
- The Little Book of Rust Macros: [danielkeep.github.io](https://danielkeep.github.io/tlborm/book/)
