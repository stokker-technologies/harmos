<!-- Rendered by stkr project update from .agents/skills/harmos-authoring/references/operations.md; sha256:af37168f08cc17d3c832dac78e296c2de03712883cf61089e74ddc9688740e8e -->

# Operations

## Vertical Authoring

A portable request is separate from its operation declaration. Put request,
response, declaration and runtime implementation together in the operation's
concern directory. Shared product values stay in the domain. This example assumes
`device::Error` converts `harmos::DependencyError`:

```rust
#[cfg(feature = "runtime")]
use {device::{Device, Error}, harmos::{Operation, OperationContext, Response}, std::sync::Arc};

#[harmos::message]
pub struct ReadRequest { pub channel: u32 }

#[harmos::message]
pub struct ReadResponse { pub value: f64 }

#[harmos::operation(id = "read", request = ReadRequest, response = ReadResponse,
    policy = crate::POLICY, infer_request)]
pub struct Read {
    #[cfg(feature = "runtime")]
    device: Arc<Device>,
}

#[cfg(feature = "runtime")]
impl Operation for Read {
    type Request = ReadRequest;
    type Response = ReadResponse;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: OperationContext) -> Result<Self, Error> {
        Ok(Self { device: context.dependency::<Device>()? })
    }

    async fn run(&mut self, _: OperationContext, request: &mut ReadRequest,
        _: &mut ()) -> Result<Response<ReadResponse>, Error> {
        Ok(ReadResponse { value: self.device.read(request.channel).await? }.into())
    }
}
```

The operations crate uses `harmos` with `default-features = false` and
`runtime = ["harmos/runtime", "dep:device"]`. Cargo feature unification can
still enable execution elsewhere in a shared graph. Keep contracts usable in
a separate build without runtime dependencies.

Harmos's own built-ins follow this layout in
`crates/harmos-operations/src/{host,lifecycle,storage,streams}/`, exposed as
`harmos::builtins`. Runtime code supplies SDK-owned state access handles;
operations never depend back on the runtime crate. Shared portable values and
reserved lifecycle protocol schemas stay in the domain. The current wire and
storage formats are v1, with no historical decoding or migration.

## Catalogs And Invocation

```rust
#[harmos::catalog]
pub enum Operations { Read(temperature::read::Read) }

let definitions = Operations::catalog();
let mut dependencies = harmos::Dependencies::new();
dependencies.insert(device)?;
use harmos::RegistrationExt;
runtime.register(Operations::registration(dependencies).lane(Lane::Interactive)).await?;
let reading = runtime.operation(ReadRequest { channel: 3 }).retry(retry).run().await?;
let pending = runtime.operation_for::<Read>(ReadRequest { channel: 3 }).start().await?;
let reading = pending.join().await?;
runtime.unregister(&harmos::QualifiedId::parse("host.read")?).await?;
```

`Application::Lanes` names the closed `Lane` enum. A batch can use
`.defaults::<Lane>()` and `.default_lane_for::<Read>(lane)`. Invocation builders
are inert until started. Dependency insertion changes only the caller's snapshot;
shared resource values remain the same `Arc<T>`. Duplicate or missing types fail
explicitly. Declare one `infer_request` per request type; omit it when requests
are reused and select the operation with `operation_for::<O>`.

## Messages And Streaming

`Message` defines `CODEC`, `encode`, `encode_into`, `encoded_size`, `decode`
and `decoding_memory`. `Portable` adds schema-aware `fingerprint()` and payload
`digest()`. Manual operation versions are replaced by fingerprints; stored
application schema and journal versions retain their own durable contracts.
Invalid static declarations fail macro checks or registration validation.

`input` and `output` independently name streamed item types. Use `()` endpoints
for unary operations, `StreamInput<T>` for input, `StreamOutput<T>` for output,
and their pair for duplex behavior. Supply typed endpoints with `.endpoints(...)`.
Retain bounded ownership and await final settlement when cancelling streams.
Live handles and reference-counted resources are execution fields rather than
portable request fields.

## Policy And Lifecycle

`OperationPolicy` has `timeout`, `attempt_timeout` and `retries`; per-call
`.retry(RetryPolicy)` leaves the declaration unchanged. Grant and cache-key hooks
inspect `&Self::Request`. Visibility and replay behavior remain declaration metadata.

`initialize(OperationContext)` constructs one fallible instance, `run` returns
`Response<T>`, and `stop` and `cleanup` settle its resources. Services implement
`Service` with the same context and lifecycle verbs and use
`ServicePolicy { stop_timeout, restart }`, including `RestartPolicy::OnFailure`.
Classify request errors where their meaning is known. Keep retry semantics
consistent with side effects and replay safety.

Local operation IDs match `[a-z][a-z0-9_]*`. The host assigns owners and qualifies
routing IDs once as `owner.local_id`. Registration validates a whole batch;
withdrawal rejects new calls and awaits cleanup. Failed cleanup retains the
closing owner; another `unregister` resumes it. Re-registration allocates a fresh
generation, fencing stale frames.
