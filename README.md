# Actor

> Independent Actors and communication between them

## Overview

A type-directed approach to handling Actors running on different threads and
communication between them through a Distributor kept in task local storage.

Uses rust-typemap internally to allow for completely type-directed dispatch
from any thread that is appropriately spawned.

## Example

```rust
// In an Actor or a thread spawned by actor::spawn
let parsed = Message::new(raw).send::<Parser>();
```

Through the use of type-directed dispatch, the Message is automatically
routed to the Parsed Actor from any actor task and without any additional
user input.

