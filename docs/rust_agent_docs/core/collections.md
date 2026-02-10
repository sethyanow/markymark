## Collections, Iterators & String Types

> **TL;DR:** Use `Vec` as default sequence, `HashMap` for key-value. Prefer iterator adapters
> over indexed loops. Know the difference between `String` (owned) and `&str` (borrowed).
> Use the entry API for map updates.

### Collection Selection Guide

| Collection | Use When | Ordered | Duplicates | Key Lookup |
|-----------|----------|---------|------------|------------|
| `Vec<T>` | Default sequence, stack, buffer | By insertion | Yes | O(n) |
| `VecDeque<T>` | Double-ended queue, ring buffer | By insertion | Yes | O(n) |
| `LinkedList<T>` | Almost never (Vec is usually better) | By insertion | Yes | O(n) |
| `HashMap<K,V>` | Key-value lookup | No | Keys: no | O(1) avg |
| `BTreeMap<K,V>` | Sorted key-value, range queries | By key | Keys: no | O(log n) |
| `HashSet<T>` | Unique values, membership test | No | No | O(1) avg |
| `BTreeSet<T>` | Sorted unique values | By value | No | O(log n) |
| `BinaryHeap<T>` | Priority queue (max-heap) | By priority | Yes | O(1) peek |

### String Types Decision Tree

```
What kind of string do you need?
├─ Owned, heap-allocated, growable?
│   └─ String
├─ Borrowed slice of UTF-8 text?
│   └─ &str
├─ OS-native string (may not be UTF-8)?
│   ├─ Owned → OsString
│   └─ Borrowed → &OsStr
├─ File system path?
│   ├─ Owned → PathBuf
│   └─ Borrowed → &Path
├─ C-compatible null-terminated string?
│   ├─ Owned (Rust → C) → CString
│   └─ Borrowed (C → Rust) → &CStr
└─ Raw bytes (not necessarily text)?
    ├─ Owned → Vec<u8>
    └─ Borrowed → &[u8]
```

**Default choice:** Accept `&str` in function parameters, return `String` when ownership is needed.

```rust
// ✅ Accept borrowed, return owned
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

### Iterator Adapter Cheatsheet

| Adapter | Purpose | Example |
|---------|---------|---------|
| `map(f)` | Transform each element | `.map(|x| x * 2)` |
| `filter(p)` | Keep elements matching predicate | `.filter(|x| x > &0)` |
| `filter_map(f)` | Transform + filter in one step | `.filter_map(|x| x.parse().ok())` |
| `flat_map(f)` | Map then flatten | `.flat_map(|line| line.split(','))` |
| `take(n)` | First n elements | `.take(5)` |
| `skip(n)` | Skip first n elements | `.skip(2)` |
| `enumerate()` | Add index | `.enumerate()` → `(usize, T)` |
| `zip(iter)` | Pair with another iterator | `.zip(other)` → `(A, B)` |
| `chain(iter)` | Concatenate iterators | `.chain(more_items)` |
| `peekable()` | Allow lookahead | `.peekable()` → `.peek()` |
| `collect()` | Consume into collection | `.collect::<Vec<_>>()` |
| `fold(init, f)` | Reduce to single value | `.fold(0, |acc, x| acc + x)` |
| `any(p)` / `all(p)` | Short-circuit bool check | `.any(|x| x > 10)` |
| `find(p)` | First match | `.find(|x| x > &10)` |
| `position(p)` | Index of first match | `.position(|x| x > 10)` |
| `sum()` / `product()` | Numeric aggregation | `.sum::<i32>()` |

### Entry API for Maps

```rust
use std::collections::HashMap;

let mut scores: HashMap<String, Vec<u32>> = HashMap::new();

// Insert-or-update pattern
scores
    .entry("Alice".to_string())
    .or_insert_with(Vec::new)
    .push(95);

// Count occurrences
let mut counts: HashMap<char, usize> = HashMap::new();
for ch in text.chars() {
    *counts.entry(ch).or_insert(0) += 1;
}
```

### Collecting Into Different Types

```rust
// Collect into Vec
let squares: Vec<i32> = (1..=5).map(|x| x * x).collect();

// Collect into HashMap
let map: HashMap<&str, usize> = vec![("a", 1), ("b", 2)].into_iter().collect();

// Collect Results — short-circuits on first Err
let results: Result<Vec<i32>, _> = strings.iter().map(|s| s.parse::<i32>()).collect();

// Collect into String
let joined: String = words.iter().copied().collect::<Vec<_>>().join(", ");
```

### References

- The Rust Book: [Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- Rust std: [Iterator](https://doc.rust-lang.org/std/iter/trait.Iterator.html)
- Related: [core/ownership.md](ownership.md) (borrowing in iterators), [core/traits.md](traits.md) (Iterator trait)
