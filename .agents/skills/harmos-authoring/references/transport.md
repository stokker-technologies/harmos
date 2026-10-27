# Transport And Generated Contracts

Declare transport exposure on the original product operation. Exposing the
same behavior over gRPC does not require another operation or forwarding handler.

This declaration fragment assumes a product-owned `Reading` and `POLICY`:

```rust
#[harmos::message]
pub struct ReadRequest { pub channel: u32 }

#[harmos::operation(
    id = "read", request = ReadRequest, response = Reading,
    policy = POLICY, infer_request,
    grpc(package = "instrument.v1", service = "Instrument", method = "Read"),
)]
pub struct Read {}
```

The runtime's gRPC facility exposes that contract. `harmos-domain` owns the
portable schema graph and metadata; `harmos-packaging` owns static schema and
binding projection; the runtime discovers currently available providers.
These are views of one declaration, not independently maintained contracts.

`Message::CODEC` identifies payload encoding; macro-authored messages use Postcard.
Custom codecs must be supported explicitly at each selected transport. Protobuf is an
explicit SDK/transport codec. Domain declarations remain host-free; SDK and
carrier protobuf codecs are optional `grpc` facilities. Select the installed
Harmos version's generation and transport features for the actual producer.

Contracts use 32-byte fingerprints, represented as 64 lowercase hex characters
in JSON, Python and TypeScript. Routing identity and lossless u64 generation
remain separate fields; generated JavaScript consumers use bigint or strings.
Stored application record and journal versions remain unchanged.

Keep generated clients matched to the producer contract revision. The consumer
owns its generated package, connection and authentication. Use the supported
packaging tools rather than manually editing generated schemas.

An external-system gRPC client library has a different responsibility from
exposing product operations over Harmos gRPC. Preserve that library's ownership
and let the product runtime compose it as a resource. Its existence does not
require a second Harmos operation contract or a new product tier.
