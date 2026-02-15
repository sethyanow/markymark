## Patterns & Idioms — Overview

Established Rust patterns and API design guidance. Read after understanding the core language.

### Files

| File | Topic | When to Read |
|------|-------|-------------|
| [idioms.md](idioms.md) | Builder, newtype, typestate, RAII, Cow, and more | Choosing the right pattern |
| [api-design.md](api-design.md) | Public API surface, naming, backwards compat | Designing library APIs |
| [anti-patterns.md](anti-patterns.md) | What NOT to do (with idiomatic fixes and real failures) | Code review, refactoring |
| [cookbook.md](cookbook.md) | Complete working recipes combining multiple concepts | Building real features |
| [async-ready.md](async-ready.md) | Make your type async-ready (Send/Sync/Pin/lifetime) | Async code, shared state |

### Reading Order

1. **idioms.md** — Know the patterns available
2. **api-design.md** — How to expose them in APIs
3. **anti-patterns.md** — What to avoid
4. **cookbook.md** — See patterns combined in real code
5. **async-ready.md** — Cross-cutting async concerns

### Common Tasks → File

| Task | File |
|------|------|
| Build a complex config object | [idioms.md](idioms.md) (Builder) |
| Add type safety to a primitive | [idioms.md](idioms.md) (Newtype) |
| Design a state machine API | [idioms.md](idioms.md) (Typestate) |
| Name methods correctly | [api-design.md](api-design.md) |
| Review code for bad patterns | [anti-patterns.md](anti-patterns.md) |
| Parse config, build a service, chain iterators | [cookbook.md](cookbook.md) |
| Make a type work in async/multi-threaded code | [async-ready.md](async-ready.md) |
