## Compiler Error Quick Reference

> **TL;DR:** Common compiler errors mapped to causes and fixes. Use `rustc --explain EXXXX`
> for full explanations.

### Ownership & Borrowing Errors

| Code | Message | Cause | Fix |
|------|---------|-------|-----|
| E0382 | Use of moved value | Value ownership transferred | Clone, borrow, or restructure |
| E0505 | Cannot move out while borrowed | Move while borrow is active | Reduce borrow scope |
| E0502 | Cannot borrow as mutable | Shared borrow exists | Restructure; use interior mutability |
| E0499 | Cannot borrow as mutable more than once | Two `&mut` to same data | Split borrows or reduce scope |
| E0507 | Cannot move out of borrowed content | Move from behind reference | Clone, `std::mem::take`, or `std::mem::replace` |
| E0515 | Cannot return reference to temporary | Reference outlives temporary | Return owned type |
| E0597 | Does not live long enough | Reference outlives referent | Extend life of data, use `'static`, or clone |
| E0106 | Missing lifetime specifier | Compiler can't infer lifetimes | Add explicit lifetime annotations |

### Type & Trait Errors

| Code | Message | Cause | Fix |
|------|---------|-------|-----|
| E0308 | Mismatched types | Type mismatch | Check types, add conversion (.into(), as) |
| E0277 | Trait not satisfied | Missing trait bound | Add bound, implement trait, or derive |
| E0412 | Cannot find type | Unknown type name | Import type, check spelling |
| E0599 | Method not found | No such method on type | Check type, import trait, check deref |
| E0609 | No field on type | Accessing nonexistent field | Check struct definition |
| E0425 | Cannot find value | Unresolved name | Import, check scope, check spelling |
| E0433 | Failed to resolve | Module/type path wrong | Check use statement |
| E0061 | Wrong number of args | Argument count mismatch | Check function signature |
| E0369 | Binary operation not supported | Operator not implemented | Implement `Add`, `PartialEq`, etc. |

### Lifetime & Reference Errors

| Code | Message | Cause | Fix |
|------|---------|-------|-----|
| E0621 | Explicit lifetime required in type | Parameters need lifetimes | Annotate lifetime on parameter |
| E0623 | Lifetime mismatch | Return type lifetime differs from args | Align lifetime annotations |
| E0716 | Temporary value dropped while borrowed | Borrow of temporary | Bind temporary to a variable first |

### Send/Sync Errors

| Message Pattern | Cause | Fix |
|----------------|-------|-----|
| "cannot be sent between threads safely" | Type is `!Send` | Use `Arc` instead of `Rc`; remove `RefCell` |
| "not safe to share between threads" | Type is `!Sync` | Use `Mutex`/`RwLock` for synchronization |
| "`dyn Future` is not `Send`" | Future holds `!Send` type across await | Scope `!Send` variables; drop before `.await` |

### Quick Diagnostic Steps

1. Read the **full error message** — Rust's errors are excellent
2. Check the **"help:" suggestion** — often gives the exact fix
3. Look at the **span** — arrows point to the problem
4. Run `rustc --explain EXXXX` — detailed explanation with examples
5. Check this table for common patterns

### References

- Error Index: [doc.rust-lang.org/error_codes](https://doc.rust-lang.org/error_codes/error-index.html)
- Related: [core/ownership.md](../core/ownership.md), [core/traits.md](../core/traits.md), [tooling/debugging.md](../tooling/debugging.md)
