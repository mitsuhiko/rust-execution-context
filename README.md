# Execution Context for Rust

This implements a .NET inspired execution context.  The idea is that something like
this could become a core language concept if it can be shown to be reasonably performant.

## What are execution contexts?

An execution context is a container for a logical call flow.  The idea is that any code
that follows the same flow of execution can access flow-local data.  An example where
this is useful is security relevant data that code might want to carry from operation
to operation without accidentally dropping it.

This gives you the most trivial example:

```rust
flow_local!(static TEST: u32 = 42);

assert_eq!(*TEST.get(), 42);
let ec = ExecutionContext::capture();
TEST.set(23);

assert_eq!(*TEST.get(), 23);
ec.run(|| {
    assert_eq!(*TEST.get(), 42);
});
```

Execution contexts can be forwarded to other threads and an API is provided to
temporarily or permanently suppress the flow propagation.
