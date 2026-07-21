# Rust Guidelines — DOs & DON'Ts

Concise cheat-sheet derived from *The Rust Programming Language* book.

---

## Ownership & Move Semantics

### DO

- **Transfer ownership explicitly when a value should have a single owner.**
  Most types move on assignment. Design APIs so the transfer is obvious, not accidental.

- **Derive `Clone`/`Copy` only on cheap, small types (scalars, small structs).**
  `Copy` implies bitwise duplication is semantically correct and cheap. Heap-owning or resource-managing types must not be `Copy`.

- **Use `clone()` intentionally and visibly when you need a deep copy.**
  Every `.clone()` is a cost decision. Make it a conscious choice, not a reflex to quiet the compiler.

### DON'T

- **Assume values are copied — most types move by default.**
  Only types that implement `Copy` are duplicated on assignment. Everything else transfers ownership and invalidates the source.

- **Derive `Copy` on types that hold heap data or file handles.**
  Bitwise copy of a `Vec`, `String`, or file descriptor creates double-free or use-after-close bugs that the compiler prevents by not auto-deriving `Copy`.

- **Scatter `.clone()` to silence the borrow checker without understanding why.**
  If the borrow checker complains, the ownership model is telling you something. Cloning hides the real design issue and adds unnecessary allocations.

---

## Borrowing & References

### DO

- **Prefer `&T` (shared ref) when you only need to read.**
  Shared references allow multiple simultaneous readers and place no exclusivity constraint on the caller.

- **Keep borrow scopes as short as possible.**
  The shorter a borrow lives, the less it constrains what else can use the value. Drop references before mutating.

- **Reborrow (`&*x`) instead of moving a reference when possible.**
  Reborrowing creates a shorter-lived reference from an existing one, keeping the original usable afterward.

- **Accept `&str` / `&[T]` in function params for maximum flexibility.**
  Callers can pass owned types, slices, or literals without conversion. Borrowing in parameters is the idiomatic default.

### DON'T

- **Take `&mut T` when `&T` suffices.**
  A mutable reference is exclusive — it blocks all other access to the value. Request only the access you need.

- **Hold a borrow across an `await` point or a long-lived scope unnecessarily.**
  Long borrows prevent the owner from being used elsewhere and can cause confusing lifetime errors, especially in async code.

- **Create aliasing `&mut` references.**
  Rust's core safety guarantee forbids multiple mutable references to the same data. The compiler enforces this, but fighting it wastes time.

- **Require `String` / `Vec<T>` in params when you don't need ownership.**
  Forcing owned types makes callers allocate unnecessarily. Accept borrows unless the function genuinely needs to own the data.

---

## Lifetimes

### DO

- **Let lifetime elision do its job — annotate only when the compiler asks.**
  Elision rules handle the majority of cases. Adding lifetimes preemptively clutters signatures without adding safety.

- **Name lifetimes descriptively (`'input`, `'cfg`) in complex signatures.**
  When multiple lifetimes interact, descriptive names communicate which data each lifetime tracks.

- **Prefer owned data in structs unless the borrowed data clearly outlives the struct.**
  Owned fields are simpler to reason about, compose freely, and don't propagate lifetime parameters to every user of the struct.

### DON'T

- **Add explicit lifetimes everywhere "just in case."**
  Unnecessary annotations add noise and can mislead readers into thinking the relationships are more complex than they are.

- **Default to `'a`, `'b`, `'c` when meaningful names would clarify intent.**
  Single-letter lifetimes are fine for trivial cases but obscure meaning when a signature has two or more lifetime parameters.

- **Store references in structs as a first instinct.**
  Borrowing in structs forces the struct to be parameterized by a lifetime, which ripples through every function that touches it. Start with owned data and borrow only when profiling or design demands it.

---

## Structs, Enums & Pattern Matching

### DO

- **Model states as enum variants with data, not boolean flags.**
  Enums make invalid states unrepresentable. The compiler enforces that every state transition is handled.

- **Use `match` exhaustively — let the compiler enforce completeness.**
  Exhaustive matching means adding a variant is a compile error everywhere it matters, not a silent bug.

- **Destructure in `match` arms to extract fields.**
  Destructuring binds fields directly, making each arm self-contained and avoiding repeated field access.

- **Prefer `if let` / `let else` for single-variant checks.**
  When you only care about one variant and want to skip the rest, `if let` is clearer than a `match` with a `_ => ()` arm.

### DON'T

- **Use `is_active: bool, is_pending: bool` to represent mutually exclusive states.**
  Boolean flags can express impossible combinations (active *and* pending). An enum prevents this at the type level.

- **Use `_ =>` as a catch-all when you could name remaining variants explicitly.**
  A wildcard arm silently swallows new variants added later. Naming each variant forces you to handle new cases.

- **Match and then access fields separately afterward.**
  Splitting the match from the field access duplicates the knowledge of which variant you're in and risks accessing the wrong field.

- **Write a full `match` when you only care about one variant.**
  A five-arm match where four arms do nothing is visual noise. `if let` / `let else` express "I care about this one case" directly.

---

## Error Handling

### DO

- **Return `Result<T, E>` from functions that can fail.**
  `Result` makes failure explicit in the type signature. Callers must acknowledge it — they can't accidentally ignore it.

- **Use `?` to propagate errors up the call chain.**
  The `?` operator converts and propagates errors in one character, keeping the happy path visually clean.

- **Create domain-specific error enums for public APIs.**
  Typed errors let callers match on specific failure modes and handle each one appropriately.

- **Use `unwrap()` / `expect()` in tests and examples where failure means a bug.**
  In test code, a panic on unexpected failure is the right behavior — it fails the test immediately with a clear message.

- **Use `anyhow` for applications, `thiserror` for libraries.**
  `anyhow` is ergonomic for top-level error aggregation. `thiserror` generates structured error types that library consumers can pattern-match.

### DON'T

- **Use `unwrap()` / `expect()` in library or production code.**
  An `unwrap` in production is an unrecoverable panic waiting to happen. Propagate errors and let the caller decide.

- **Manually match every `Result` just to re-wrap the error.**
  If you're writing `match result { Ok(v) => Ok(v), Err(e) => Err(MyError::from(e)) }`, use `?` with a `From` impl instead.

- **Return `Box<dyn Error>` from library code.**
  Dynamic error types erase the specific failure information that callers need to handle errors intelligently.

- **Silently ignore errors with `let _ = ...` unless the discard is intentional and documented.**
  Discarding a `Result` hides failures. If you genuinely don't care, make that choice visible with a comment.

- **Mix `anyhow` and `thiserror` in the same crate without reason.**
  They serve different purposes. Mixing them creates confusion about whether errors are meant to be matched or just reported.

---

## Traits & Generics

### DO

- **Define small, focused traits (one responsibility).**
  Small traits compose well and are easy to implement. Callers can require exactly the capabilities they need.

- **Use trait bounds (`T: Display + Debug`) to constrain generics.**
  Bounds document what capabilities a generic function requires and produce clear errors when they're missing.

- **Prefer `impl Trait` in return position for simple cases.**
  `impl Trait` keeps the concrete type private, allowing the implementation to change without breaking callers.

- **Use `where` clauses when bounds get long.**
  Moving bounds to a `where` clause keeps the function signature readable and the bounds scannable.

- **Implement standard traits (`Display`, `Debug`, `From`, `Default`) when meaningful.**
  Standard traits integrate your types into the ecosystem — formatting, error conversion, default construction — for free.

### DON'T

- **Create "god traits" with many unrelated methods.**
  A trait with ten methods is hard to implement and impossible to compose. Split by responsibility.

- **Leave generics unbounded when you know the required capabilities.**
  Unbounded generics defer errors to the function body, producing confusing messages. Declare what you need upfront.

- **Reach for `Box<dyn Trait>` when static dispatch works fine.**
  Dynamic dispatch adds indirection and prevents inlining. Use it when you need heterogeneous collections or runtime polymorphism, not as a default.

- **Cram complex bounds inline — it hurts readability.**
  `fn foo<T: AsRef<str> + Clone + Send + Sync + 'static>(x: T)` is hard to parse. Move it to `where`.

- **Implement `Deref`/`DerefMut` to simulate inheritance.**
  `Deref` is for smart pointer semantics, not for "my struct should act like its inner field." Misusing it creates confusing method resolution.

---

## Collections & Iterators

### DO

- **Prefer iterator chains (`.iter().map().filter().collect()`) over manual loops.**
  Iterators are zero-cost abstractions that the compiler optimizes aggressively. They also prevent off-by-one errors.

- **Use `collect::<Vec<_>>()` with a turbofish or let type inference resolve the target.**
  The turbofish makes the intent explicit at the call site. Either approach works; pick one and be consistent.

- **Use the `entry()` API for conditional insert-or-update on `HashMap`.**
  `entry()` looks up the key once. The check-then-insert pattern hashes twice and races in concurrent code.

- **Prefer `&[T]` slices over `&Vec<T>` in function signatures.**
  A slice is strictly more general — it accepts arrays, vectors, and subslices. `&Vec<T>` adds nothing.

### DON'T

- **Write `for i in 0..vec.len()` and index manually when iterators work.**
  Manual indexing is error-prone (off-by-one, bounds panics) and prevents the compiler from eliding bounds checks.

- **Collect into an intermediate `Vec` only to iterate it again immediately.**
  Chaining iterators lazily avoids the temporary allocation. Only collect when you need a concrete collection.

- **Check `contains_key()` then `insert()` — it double-hashes.**
  Two separate lookups for the same key is wasteful. The `entry()` API does both in one operation.

- **Require `&Vec<T>` when a slice is sufficient.**
  `&Vec<T>` forces callers to have a `Vec` on hand. `&[T]` accepts anything contiguous.

---

## Strings

### DO

- **Accept `&str` in functions, return `String` when building new data.**
  `&str` is the universal string borrow — it accepts `&String`, literals, and slices. Return `String` when the function creates new data.

- **Use `format!()` for readable string construction.**
  `format!` handles interpolation, padding, and type formatting in one readable expression.

- **Use `to_owned()` to go from `&str` to `String`.**
  `to_owned()` directly expresses "I want an owned copy of this borrowed data" without going through `Display`.

- **Remember strings are UTF-8: use `.chars()` for characters, `.bytes()` for raw bytes.**
  Rust strings are not arrays of characters. Iterating by char handles multi-byte code points correctly.

### DON'T

- **Accept `&String` — it's almost never what you want.**
  `&String` is a reference to an owned string. `&str` is strictly more general and avoids forcing callers to own a `String`.

- **Chain `.push_str()` / `+` repeatedly for complex formatting.**
  Repeated concatenation is hard to read and may reallocate multiple times. `format!` does it in one shot.

- **Confuse `to_string()` with `to_owned()` for `&str` to `String`.**
  `to_string()` goes through the `Display` trait's formatting machinery. `to_owned()` is a direct copy and communicates intent more clearly.

- **Index strings by byte position (`s[0]`).**
  Byte indexing panics if the index lands inside a multi-byte character. Use `.chars().nth()` or slicing with care.

---

## Closures

### DO

- **Let the compiler infer closure argument/return types.**
  Closures are usually short-lived and used in context where the types are obvious. Explicit annotations add noise.

- **Use `move` closures to transfer ownership into threads or `'static` contexts.**
  `move` ensures the closure owns its captured data, making it safe to send across thread boundaries or store indefinitely.

- **Prefer `Fn` > `FnMut` > `FnOnce` in trait bounds (most to least restrictive for callers).**
  `Fn` is the most permissive for callers — any closure that can be called by shared reference qualifies. Only require `FnMut` or `FnOnce` when the closure genuinely needs mutation or consumption.

### DON'T

- **Annotate closure types unless required for disambiguation.**
  Explicit type annotations on closures are rarely needed and make the code look heavier than it is.

- **Forget that `move` captures all referenced variables, not just the ones you need.**
  If a closure mentions five variables and you add `move`, all five are moved in. Clone what you need beforehand to avoid moving too much.

- **Require `FnOnce` when the closure will be called multiple times.**
  `FnOnce` consumes the closure on the first call. If you call it in a loop, you need `Fn` or `FnMut`.

---

## Smart Pointers

### DO

- **Use `Box<T>` for heap allocation and recursive types.**
  `Box` provides a known-size indirection. Recursive types need it because the compiler can't determine their size otherwise.

- **Use `Rc<T>` / `Arc<T>` for shared ownership (single-threaded / multi-threaded).**
  Reference counting is the right tool when multiple owners need to keep data alive and there's no clear single owner.

- **Pair `Arc` with `Mutex` or `RwLock` for shared mutable state across threads.**
  `Arc` handles the shared ownership; `Mutex`/`RwLock` handles the interior mutability. Neither works alone.

- **Prefer `Cow<'_, str>` when a function sometimes borrows, sometimes owns.**
  `Cow` defers the allocation to the cases that actually need it, keeping the borrow path zero-cost.

### DON'T

- **Box everything — stack allocation is cheaper when size is known.**
  Heap allocation has overhead (allocator call, indirection, cache miss). Use the stack for values with known, reasonable size.

- **Use `Rc<T>` across threads — it's not `Send`.**
  `Rc` uses non-atomic reference counting. Sharing it across threads is a data race. Use `Arc` for multi-threaded shared ownership.

- **Nest `Mutex` inside `Mutex` — it invites deadlocks.**
  Acquiring an outer lock then an inner lock in inconsistent order across call sites is a classic deadlock. Flatten the structure or use a single lock.

- **Allocate unconditionally when `Cow` would avoid the clone in the common path.**
  If most calls just pass data through unchanged, `Cow` lets you skip the allocation entirely and only clone when mutation is needed.

---

## Concurrency

### DO

- **Use message passing (`mpsc`, `crossbeam` channels) as the default communication model.**
  Channels decouple producers and consumers, eliminate shared state, and make data flow explicit and testable.

- **Scope threads (`std::thread::scope`) when threads don't need to outlive the caller.**
  Scoped threads borrow from the parent stack safely — no `Arc`, no `'static` bounds, no manual join management.

- **Prefer `RwLock` over `Mutex` when reads vastly outnumber writes.**
  `RwLock` allows concurrent readers, only blocking when a writer is active. For read-heavy workloads, this eliminates unnecessary contention.

- **Use `Send` / `Sync` bounds to enforce thread safety at compile time.**
  These marker traits are Rust's zero-cost concurrency guarantee. The compiler checks them automatically — lean on that.

### DON'T

- **Default to shared mutable state (`Mutex`) when message passing is clearer.**
  Shared state requires careful locking discipline. Channels let you reason about concurrency as data flow, not lock ordering.

- **Spawn threads and leak join handles.**
  An unjoined thread runs until it finishes or the process exits. You lose error reporting, resource cleanup, and backpressure.

- **Hold a lock across an `await` boundary (for `std::sync::Mutex`).**
  A `std::sync::Mutex` guard is not `Send`. Holding it across an await can block the executor thread and cause deadlocks. Use `tokio::sync::Mutex` if you must hold across awaits.

- **Implement `Send` / `Sync` manually unless you deeply understand the invariants.**
  Incorrect manual implementations of these traits bypass the compiler's safety checks and can introduce undefined behavior.

---

## Modules & Crate Structure

### DO

- **Keep `pub` surface area minimal — default to private.**
  Everything private by default means you can refactor internals freely. Public items are promises to every consumer.

- **Use `pub(crate)` for crate-internal sharing that shouldn't leak to users.**
  `pub(crate)` gives sibling modules access without committing to a stable public API.

- **Organize by responsibility: one module = one concern.**
  Modules should be cohesive. If you can't describe what a module does in one sentence, it's doing too much.

- **Re-export key types from the crate root or a `prelude` module.**
  Short import paths reduce friction for users and make the public API discoverable from one place.

### DON'T

- **Make everything `pub` for convenience.**
  Over-exposing internals locks you into supporting them. Every `pub` item is a maintenance commitment.

- **Expose internal helpers as `pub` just because another module needs them.**
  If the helper is only useful within the crate, `pub(crate)` keeps it accessible without leaking it externally.

- **Create deeply nested module hierarchies that mirror directory structure for its own sake.**
  Deep nesting creates long import paths and navigation friction. Organize for clarity, not symmetry.

- **Force users to navigate deep paths (`crate::a::b::c::Type`).**
  Deep paths are a symptom of missing re-exports. Surface important types at a convenient depth.

---

## Testing

### DO

- **Put unit tests in a `#[cfg(test)] mod tests` block inside the source file.**
  Colocated tests have access to private items and stay in sync with the code they test.

- **Use `#[should_panic(expected = "...")]` to test expected panics.**
  The `expected` string ensures the test fails for the right reason, not an unrelated panic.

- **Test edge cases: empty inputs, boundary values, error paths.**
  Happy-path-only tests miss the bugs that ship. Edge cases are where most real failures occur.

- **Use `assert_eq!` / `assert_ne!` over plain `assert!` — better error messages.**
  `assert_eq!` prints both values on failure. `assert!(a == b)` just says "assertion failed."

- **Put integration tests in the `tests/` directory.**
  Integration tests in `tests/` run as separate binaries, exercising the crate's public API the way an external consumer would.

### DON'T

- **Create separate test files for unit tests — that's for integration tests.**
  Unit tests belong next to the code they test. Separate files lose access to private items and add navigational overhead.

- **Write tests that just `assert!(true)`.**
  A test that can't fail provides no value. Every test should assert a meaningful property of the code.

- **Only test the happy path.**
  If your tests don't cover error conditions, boundary cases, and empty inputs, you're not testing — you're demonstrating.

- **Ignore test output — read what failed and why.**
  Test output includes the values, locations, and messages that point directly at the bug. Skipping it means debugging blind.

- **Mix integration and unit tests in the same location.**
  Each has a distinct purpose and scope. Mixing them blurs the line between testing internals and testing the public contract.

---

## Type System & Conversions

### DO

- **Use `From` / `Into` for infallible conversions.**
  `From` is the standard way to express "this type can always be constructed from that type." Implement `From` and get `Into` for free.

- **Use `AsRef<T>` / `AsMut<T>` for cheap reference-to-reference conversions.**
  `AsRef` signals that the conversion is essentially free — a pointer cast or field access, not a computation.

- **Use newtypes (`struct Meters(f64)`) to enforce type safety.**
  Newtypes prevent mixing up arguments of the same primitive type. The compiler catches `Meters` vs `Seconds` at zero runtime cost.

- **Prefer turbofish (`::<Type>`) over type annotations when both work.**
  Turbofish keeps the type information at the call site where ambiguity occurs, rather than on a separate `let` binding.

### DON'T

- **Implement `From` for conversions that can fail — use `TryFrom` instead.**
  `From` implies infallibility. A fallible conversion in `From` panics or silently loses data. `TryFrom` returns a `Result`.

- **Implement expensive conversions via `AsRef` — it implies zero-cost.**
  Callers expect `AsRef` to be trivially cheap. If your conversion allocates or computes, use `From` or a named method instead.

- **Pass raw primitives when domain types would prevent mixing up arguments.**
  `fn transfer(f64, f64)` accepts `(amount, fee)` and `(fee, amount)` identically. Newtypes make the compiler catch the swap.

- **Let ambiguous type inference produce confusing errors — be explicit.**
  When the compiler can't infer a type, the resulting error can be cryptic. A turbofish or annotation resolves it at the source.

---

## Cargo & Dependencies

### DO

- **Pin dependency versions in applications (`=1.2.3` or lock file).**
  Pinning ensures reproducible builds. The lock file captures the exact versions tested and deployed.

- **Enable only the feature flags you need.**
  Unused features pull in extra code, increase compile times, and expand the attack surface.

- **Run `cargo clippy` and fix warnings — it catches real bugs.**
  Clippy's lints encode community knowledge about common mistakes, performance pitfalls, and unidiomatic patterns.

- **Use `cargo fmt` consistently across the project.**
  Automated formatting eliminates style debates and keeps diffs focused on logic changes, not whitespace.

### DON'T

- **Use wildcard versions (`*`) in `Cargo.toml`.**
  Wildcards accept any version, including breaking changes. A minor release with an incompatible API change will break your build with no warning.

- **Pull in a large crate with all features for one small function.**
  Every dependency is compile time, binary size, and supply-chain risk. If you need one function, check if a smaller crate or hand-written code suffices.

- **Suppress clippy lints without understanding them (`#[allow(...)]` everywhere).**
  Blanket suppression hides real issues alongside false positives. Understand each lint, fix what's real, and suppress only what's genuinely inapplicable.

- **Argue about formatting — let `rustfmt` decide.**
  Formatting is a solved problem. Spending time on brace placement or import ordering is time not spent on correctness.
