## Migration Bridges — Coming from Python / TypeScript

> **TL;DR:** Side-by-side translations for developers moving from Python or TypeScript to Rust.
> Covers the mental model shifts, not just syntax differences.

### Mental Model Shifts

These are the conceptual changes that trip up Python/TS developers most:

| Python/TS Concept | Rust Equivalent | Key Difference |
|-------------------|-----------------|----------------|
| Garbage collection | Ownership + Drop | You decide when memory is freed (via scope) |
| Everything is a reference | Move by default | Assignment transfers ownership unless you borrow |
| `null` / `None` / `undefined` | `Option<T>` | Compiler forces you to handle the missing case |
| Exceptions | `Result<T, E>` | Errors are values, not control flow |
| Duck typing / structural typing | Traits + generics | Behavior is declared and checked at compile time |
| Classes with inheritance | Structs + traits + enums | Composition over inheritance; enums for variants |
| `async/await` (event loop) | `async/await` (runtime) | Similar syntax, but you choose the runtime (tokio) |
| Mutable by default | Immutable by default | `let` is immutable; `let mut` opts into mutation |
| Dynamic dispatch | Static dispatch (default) | Generics monomorphize; `dyn Trait` for dynamic |

### Variable Binding & Mutability

**Python:**
```python
x = 5
x = "hello"     # type can change
items = [1, 2]
items.append(3)  # always mutable
```

**TypeScript:**
```typescript
let x = 5;
x = 10;           // mutable by default with let
const y = [1, 2];
y.push(3);         // const prevents reassignment, not mutation
```

**Rust:**
```rust
let x = 5;
// x = 10;         // ERROR: immutable by default
let mut x = 5;
x = 10;            // OK: opted into mutation

let items = vec![1, 2];
// items.push(3);   // ERROR: items is not mut
let mut items = vec![1, 2];
items.push(3);     // OK

// Shadowing — rebind with new type (common Rust pattern)
let x = "hello";   // shadows the integer x; this is idiomatic
```

### Null Handling

**Python:**
```python
def find_user(id: int) -> User | None:
    user = db.get(id)
    if user is None:
        return None
    return user

# Caller can forget to check None — runtime crash
name = find_user(42).name  # AttributeError if None
```

**TypeScript:**
```typescript
function findUser(id: number): User | undefined {
    return db.get(id);
}
// With strictNullChecks, TS catches some of these
const name = findUser(42)?.name;  // optional chaining
```

**Rust:**
```rust
fn find_user(id: u64) -> Option<User> {
    db.get(id)  // returns Option<User>
}

// Compiler FORCES you to handle None:
let name = find_user(42)
    .map(|u| u.name.clone())       // transform if Some
    .unwrap_or("unknown".into());  // default if None

// Or with pattern matching:
match find_user(42) {
    Some(user) => println!("{}", user.name),
    None => println!("not found"),
}

// Or with if-let for simple cases:
if let Some(user) = find_user(42) {
    println!("{}", user.name);
}
```

### Error Handling

**Python:**
```python
try:
    data = json.loads(text)
    config = validate(data)
except json.JSONDecodeError as e:
    print(f"Bad JSON: {e}")
except ValidationError as e:
    print(f"Invalid: {e}")
```

**TypeScript:**
```typescript
try {
    const data = JSON.parse(text);
    const config = validate(data);
} catch (e) {
    // What type is e? Could be anything
    console.error(e);
}
```

**Rust:**
```rust
// Errors are typed values, not exceptions
fn load_config(text: &str) -> Result<Config, ConfigError> {
    let data: Value = serde_json::from_str(text)?;  // ? propagates error
    let config = validate(data)?;
    Ok(config)
}

// Caller handles errors explicitly:
match load_config(text) {
    Ok(config) => use_config(config),
    Err(ConfigError::Parse(e)) => eprintln!("Bad JSON: {e}"),
    Err(ConfigError::Validation(e)) => eprintln!("Invalid: {e}"),
}
```

**Key difference:** `?` replaces `try/catch` — it short-circuits and returns the error.
No hidden control flow. The return type documents every possible failure.

### Collections & Iteration

**Python:**
```python
# List comprehension
evens = [x * 2 for x in range(10) if x % 2 == 0]

# Dict comprehension
word_counts = {w: text.count(w) for w in words}
```

**TypeScript:**
```typescript
const evens = Array.from({length: 10}, (_, i) => i)
    .filter(x => x % 2 === 0)
    .map(x => x * 2);
```

**Rust:**
```rust
// Iterator chains (lazy, zero-cost)
let evens: Vec<i32> = (0..10)
    .filter(|x| x % 2 == 0)
    .map(|x| x * 2)
    .collect();

// HashMap construction
use std::collections::HashMap;
let word_counts: HashMap<&str, usize> = words
    .iter()
    .map(|w| (*w, text.matches(w).count()))
    .collect();
```

**Key difference:** Rust iterators are lazy (nothing happens until `.collect()` or `.for_each()`).
This enables zero-cost chains — the compiler fuses the operations into a single loop.

### Classes vs Structs + Traits

**Python:**
```python
class Animal:
    def __init__(self, name: str):
        self.name = name

    def speak(self) -> str:
        raise NotImplementedError

class Dog(Animal):
    def speak(self) -> str:
        return f"{self.name} says Woof!"
```

**TypeScript:**
```typescript
interface Animal {
    name: string;
    speak(): string;
}

class Dog implements Animal {
    constructor(public name: string) {}
    speak() { return `${this.name} says Woof!`; }
}
```

**Rust:**
```rust
// Trait = interface (behavior contract)
trait Animal {
    fn name(&self) -> &str;
    fn speak(&self) -> String;
}

// Struct = data
struct Dog {
    name: String,
}

// impl = connect behavior to data
impl Animal for Dog {
    fn name(&self) -> &str {
        &self.name
    }
    fn speak(&self) -> String {
        format!("{} says Woof!", self.name)
    }
}
```

**No inheritance.** Use composition:
```rust
struct PoliceDog {
    dog: Dog,
    badge_number: u32,
}

impl Animal for PoliceDog {
    fn name(&self) -> &str { self.dog.name() }
    fn speak(&self) -> String { self.dog.speak() }
}
```

### Async/Await

**Python:**
```python
import asyncio

async def fetch_data(url: str) -> dict:
    async with aiohttp.ClientSession() as session:
        async with session.get(url) as response:
            return await response.json()

async def main():
    results = await asyncio.gather(
        fetch_data("https://api.example.com/a"),
        fetch_data("https://api.example.com/b"),
    )
```

**TypeScript:**
```typescript
async function fetchData(url: string): Promise<any> {
    const response = await fetch(url);
    return response.json();
}

async function main() {
    const [a, b] = await Promise.all([
        fetchData("https://api.example.com/a"),
        fetchData("https://api.example.com/b"),
    ]);
}
```

**Rust:**
```rust
async fn fetch_data(url: &str) -> Result<Value, reqwest::Error> {
    let response = reqwest::get(url).await?;
    let data = response.json().await?;
    Ok(data)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (a, b) = tokio::join!(
        fetch_data("https://api.example.com/a"),
        fetch_data("https://api.example.com/b"),
    );
    let a = a?;  // handle errors after join
    let b = b?;
    Ok(())
}
```

**Key differences:**
- You choose the runtime (`tokio`, `async-std`) — it's not built in
- `?` propagates errors instead of exceptions
- `tokio::join!` is like `Promise.all` / `asyncio.gather`
- Spawned tasks require `Send + 'static` (no borrowed data)

### String Handling

| Python/TS | Rust | Notes |
|-----------|------|-------|
| `str` / `string` | `String` (owned) | Heap-allocated, growable |
| string literal | `&str` (borrowed) | Points into binary or another String |
| `f"Hello {name}"` | `format!("Hello {name}")` | Returns `String` |
| `s.split(",")` | `s.split(',')` | Returns iterator, not list/array |
| `s.strip()` | `s.trim()` | Returns `&str` (no allocation) |
| `",".join(list)` | `vec.join(",")` | Returns `String` |
| `s[0:5]` | `&s[0..5]` | Panics if not on char boundary! |
| `s[0]` | `s.chars().nth(0)` | Rust strings are UTF-8, not indexable |

**The big gotcha:** Python/TS strings are sequences of characters. Rust strings are
sequences of UTF-8 bytes. `&s[0..5]` slices bytes, not characters. For character-aware
operations, use `.chars()`.

### Common Traps for Python/TS Developers

| Trap | What You'd Do in Python/TS | What to Do in Rust |
|------|---------------------------|-------------------|
| "I'll just clone everything" | Everything is a reference, no cost | Clone has a cost — restructure borrows first |
| "I'll use `unwrap()` for now" | Exception will show a stack trace | `unwrap()` panics with no context — use `?` or `expect("why")` |
| "I need a class hierarchy" | Inheritance chain | Use enums for variants, traits for behavior |
| "I'll store references in a struct" | Objects live on the heap, GC handles it | Lifetime annotations required — consider owned data |
| "I'll use a global variable" | Module-level variable | `static` requires `Sync`; use `OnceLock` or `lazy_static!` |
| "I'll mutate while iterating" | Modify list during for-each | Borrow checker prevents this — use `retain`, `drain`, or index loop |
| "I'll return a closure" | Functions are first-class | `impl Fn()` or `Box<dyn Fn()>` — see [closures.md](../core/closures.md) |

### Ownership — The Core Concept Python/TS Lacks

In Python/TS, you never think about who "owns" data because the garbage collector handles it.
In Rust, every piece of data has exactly one owner. When the owner goes out of scope, the data
is freed. This is the single biggest mental shift.

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1;      // s1's ownership MOVED to s2
    // println!("{s1}"); // ERROR: s1 is no longer valid

    let s3 = s2.clone();  // explicit copy — both s2 and s3 are valid
    println!("{s2} {s3}"); // OK
}

// When main() ends: s2 and s3 are dropped (freed) automatically
// No garbage collector needed — deterministic cleanup
```

**The payoff:** No GC pauses, no memory leaks, no use-after-free, no data races.
The compiler catches all of these at compile time.

### References

- Start here: [../core/ownership.md](../core/ownership.md) (the foundation)
- Errors: [../core/errors.md](../core/errors.md) (Result and ? operator)
- Traits: [../core/traits.md](../core/traits.md) (replacing classes)
- Closures: [../core/closures.md](../core/closures.md) (replacing lambdas)
- Collections: [../core/collections.md](../core/collections.md) (Vec, HashMap, iterators)
- Async: [../advanced/async.md](../advanced/async.md) (tokio runtime)
