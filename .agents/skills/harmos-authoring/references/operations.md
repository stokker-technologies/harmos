# Operations

## Portable Declarations

Use `#[harmos_domain::message]` for portable messages and
`#[harmos_domain::operation]` on the owning request. The domain declaration owns
identity, request, final reply, streaming item types, version, policy and
visibility. Host-side declarations can use the corresponding attributes
through `harmos`.

This fragment assumes a product-owned `POLICY`:

```rust
#[harmos_domain::message]
pub struct Reading {
    pub value: f64,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "instrument.read",
    response = Reading,
    policy = POLICY,
    version = 1,
)]
pub struct Read {
    pub channel: u32,
}

#[harmos_domain::catalog]
pub enum Operations {
    Read(Read),
}
```

`response` names the final reply; `input` and `output` independently declare
streamed item types. With neither endpoint the operation is unary; either or
both directions can stream.
Messages contain portable values such as scalars, strings, bytes, nested
messages, optional values and sequences. `Box<T>` can break a recursive
Rust representation without changing the nested message's wire identity.
Live handles and reference-counted execution resources are not message fields.

In a reusable product this belongs in `product-domain/src/operations/`.
In a self-contained crate the same declaration can share a vertical operation
file with its handler. Shared models have their own owner instead of being
copied between requests.

## Implementations And Catalogs

A runtime handler implements the original request type using the applicable
execution API. Carrier authoring attributes such as
`#[harmos_sidecar::operation]` and `#[harmos_guest::operation]` generate
handler machinery from the supported lifecycle and run signatures.

A declaration catalog contains request types. A separate handler catalog
contains implementation types. Both can use the catalog macro, but they serve
different purposes. Binding selects the implementation and its resource owner;
it does not redefine policy, identity or schemas.

Keep application behavior in the runtime and device/client behavior in its
resource owner. A plugin should not wrap each existing operation in another
operation merely to reach the runtime. Concrete carrier contexts can differ:
share portable behavior and use compatible handler bindings rather than
assuming a native handler type implements a guest interface.

## Policy And Outcomes

Declare timeout, retry, visibility and replay behavior with the operation.
Choose retry semantics according to side effects and idempotency, not just
transport errors. Preserve cancellation and stream completion through handler
cleanup; an acknowledgement of admission is not necessarily durable completion.

Cross-provider use requires shared visibility and compatible complete
contracts. Matching names or a linked Rust request type alone grants no
execution authority. Classify errors at the owner that understands them and
preserve source failures for callers and diagnostics.
