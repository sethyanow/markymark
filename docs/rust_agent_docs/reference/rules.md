## Rules Quick Reference

> **TL;DR:** Core Rust rules in table form for fast lookup. No explanations —
> see linked files for details.

### Ownership Rules

| # | Rule | Detail |
|---|------|--------|
| 1 | Each value has exactly one owner | [core/ownership.md](../core/ownership.md) |
| 2 | When the owner goes out of scope, the value is dropped | |
| 3 | Ownership can be transferred (moved); old binding invalidated | |

### Borrowing Rules

| Rule | Meaning |
|------|---------|
| One `&mut T` XOR many `&T` | Never both at the same time |
| References must always be valid | No dangling references |
| Borrows end at last use (NLL) | Non-lexical lifetimes since Rust 2018 |

### Lifetime Elision Rules

Applied in order by the compiler. If all output lifetimes resolved → no annotation needed.

| # | Rule | Applied To |
|---|------|-----------|
| 1 | Each input reference gets its own lifetime | `fn f(x: &T, y: &U)` → `fn f<'a,'b>(...)` |
| 2 | If exactly one input lifetime, assign to all outputs | `fn f(x: &T) -> &U` → same `'a` |
| 3 | If one input is `&self`/`&mut self`, assign its lifetime to all outputs | Methods |

### Operators and Required Traits

| Operator | Trait | Method |
|----------|-------|--------|
| `+` | `Add` | `add(self, rhs)` |
| `-` | `Sub` | `sub(self, rhs)` |
| `*` | `Mul` | `mul(self, rhs)` |
| `/` | `Div` | `div(self, rhs)` |
| `%` | `Rem` | `rem(self, rhs)` |
| `-x` | `Neg` | `neg(self)` |
| `==`, `!=` | `PartialEq` | `eq(&self, &rhs)` |
| `<`, `<=`, `>`, `>=` | `PartialOrd` | `partial_cmp(&self, &rhs)` |
| `&` | `BitAnd` | `bitand(self, rhs)` |
| `\|` | `BitOr` | `bitor(self, rhs)` |
| `^` | `BitXor` | `bitxor(self, rhs)` |
| `<<` | `Shl` | `shl(self, rhs)` |
| `>>` | `Shr` | `shr(self, rhs)` |
| `[]` | `Index` / `IndexMut` | `index(&self, idx)` |
| `*x` | `Deref` / `DerefMut` | `deref(&self)` |
| `for x in` | `IntoIterator` | `into_iter(self)` |
| `?` | `Try` (unstable) / `From` | Early return on `Err`/`None` |

### Format String Syntax

| Placeholder | Trait | Example Output |
|-------------|-------|----------------|
| `{}` | `Display` | Human-readable |
| `{:?}` | `Debug` | Developer-readable |
| `{:#?}` | `Debug` (pretty) | Multi-line formatted |
| `{:b}` | `Binary` | `101010` |
| `{:o}` | `Octal` | `52` |
| `{:x}` / `{:X}` | `LowerHex` / `UpperHex` | `2a` / `2A` |
| `{:e}` / `{:E}` | `LowerExp` / `UpperExp` | `4.2e1` |
| `{:p}` | `Pointer` | `0x7f...` |
| `{:.3}` | `Display` | 3 decimal places |
| `{:>10}` | `Display` | Right-aligned, width 10 |
| `{:<10}` | `Display` | Left-aligned, width 10 |
| `{:^10}` | `Display` | Center-aligned, width 10 |
| `{:0>5}` | `Display` | Zero-padded, width 5 |

### Common Attributes

| Attribute | Purpose |
|-----------|---------|
| `#[derive(...)]` | Auto-implement traits |
| `#[cfg(test)]` | Compile only for tests |
| `#[cfg(feature = "X")]` | Feature-gated code |
| `#[allow(lint)]` | Suppress warning |
| `#[expect(lint, reason = "...")]` | Suppress with justification (preferred) |
| `#[must_use]` | Warn if return value unused |
| `#[non_exhaustive]` | Prevent external exhaustive matching |
| `#[repr(C)]` | C-compatible memory layout |
| `#[repr(transparent)]` | Same ABI as inner type |
| `#[inline]` | Cross-crate inlining hint |
| `#[doc(hidden)]` | Hide from documentation |
| `#[no_mangle]` | Preserve symbol name (FFI) |
| `#![deny(unsafe_code)]` | Forbid unsafe in crate |

### References

- Rust Reference: [Expressions](https://doc.rust-lang.org/reference/expressions.html)
- std::fmt: [Formatting](https://doc.rust-lang.org/std/fmt/)
- Related: [core/ownership.md](../core/ownership.md), [core/traits.md](../core/traits.md), [reference/syntax-ref.md](syntax-ref.md)
