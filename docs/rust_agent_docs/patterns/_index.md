## Patterns & Idioms — Overview

Established Rust patterns and API design guidance. Read after understanding the core language.

### Files

| File | Topic | When to Read |
|------|-------|-------------|
| [idioms.md](idioms.md) | Builder, newtype, typestate, RAII, Cow, and more | Choosing the right pattern |
| [api-design.md](api-design.md) | Public API surface, naming, backwards compat | Designing library APIs |
| [anti-patterns.md](anti-patterns.md) | What NOT to do (with idiomatic fixes) | Code review, refactoring |

### Reading Order

1. **idioms.md** — Know the patterns available
2. **api-design.md** — How to expose them in APIs
3. **anti-patterns.md** — What to avoid

### Common Tasks → File

| Task | File |
|------|------|
| Build a complex config object | [idioms.md](idioms.md) (Builder) |
| Add type safety to a primitive | [idioms.md](idioms.md) (Newtype) |
| Design a state machine API | [idioms.md](idioms.md) (Typestate) |
| Name methods correctly | [api-design.md](api-design.md) |
| Review code for bad patterns | [anti-patterns.md](anti-patterns.md) |
