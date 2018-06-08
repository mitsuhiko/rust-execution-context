# Execution Context for Rust

This implements a .NET inspired execution context.  The idea is that something like
this could become a core language concept if it can be shown to be reasonably performant.

## What are Execution Contexts?

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

## Why Are Execution Contexts?

It's pretty clear that implicit data propagation is a hotly contended topic.  For a
very long time the author of this crate was convinced that implicit data passing
through either thread locals or to have global variables is a severe case of code
smell.  However my experience has shown that there are many situations where it's
hard to avoid having a system like this.

*   Systems like distributed tracing, during-production debug tools like Sentry and
    many more require the ability to figure out at runtime what belongs together.
*   many APIs use thread locality which are becoming a problem when async/await
    are used in the language.
*   security and auditing code can be written in a safer way if security relevant
    context information is not accidentally dropped.

While obviously explicitly passing information is generally preferred in a lot of
situations, implicit data passing along the logical thread of execution is a very
potent tool.
