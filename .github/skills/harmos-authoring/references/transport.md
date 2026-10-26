<!-- Rendered by stkr project update from .agents/skills/harmos-authoring/references/transport.md; sha256:1325c8f892e4d99847680674d38057d097f22f7ee25739ef1d8ada46ded98f7a -->

# Transport And Generated Contracts

Declare transport exposure on the original product operation. Exposing the
same behavior over gRPC does not require another operation or forwarding handler.

This declaration fragment assumes a product-owned `Reading` and `POLICY`:

```rust
#[harmos_domain::message]
#[harmos_domain::operation(
    id = "instrument.read",
    response = Reading,
    policy = POLICY,
    version = 1,
    grpc(package = "instrument.v1", service = "Instrument", method = "Read"),
)]
pub struct Read {
    pub channel: u32,
}
```

The runtime's gRPC facility exposes that contract. `harmos-domain` owns the
portable schema graph and metadata; `harmos-packaging` owns static schema and
binding projection; the runtime discovers currently available providers.
These are views of one declaration, not independently maintained contracts.

`Message` supplies Postcard encoding in `harmos-domain`. Protobuf is an
explicit SDK/transport codec. Domain declarations remain host-free; SDK and
carrier protobuf codecs are optional `grpc` facilities. Select the installed
Harmos version's generation and transport features for the actual producer.

Keep generated clients matched to the producer contract revision. The consumer
owns its generated package, connection and authentication. Use the supported
packaging tools rather than manually editing generated schemas.

An external-system gRPC client library has a different responsibility from
exposing product operations over Harmos gRPC. Preserve that library's ownership
and let the product runtime compose it as a resource. Its existence does not
require a second Harmos operation contract or a new product tier.
