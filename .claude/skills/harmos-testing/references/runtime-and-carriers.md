<!-- Rendered by stkr project update from .agents/skills/harmos-testing/references/runtime-and-carriers.md; sha256:9188b1e942cb70e452ace904c9f957cad0cc59b3a9c89a8bc936fb3937710d75 -->

# Runtime And Carriers

## Runtime Composition

Exercise the product's public runtime construction and operation dispatch when
testing composition. Initialize a shared resource once and verify that the
handlers using it observe the same state. Keep resource-specific behavior in
the resource library's tests; use controlled substitutes at that library's
public boundary when a runtime test needs a particular failure.

For a native plugin composing a reusable product, consume the entry facade with
its `runtime` feature enabled. Check that the advertised catalog contains the
original domain declarations and that the handler catalog binds the intended
runtime implementations. Declaration discovery alone does not prove that a
handler is bound or dispatchable. Exercise a representative call through the
composed plugin and observe its resource effects.

Keep default domain-only facade usage independent of runtime resources. If the
product promises both domain-only and runtime-enabled consumers, cover both
dependency configurations with the project's existing check mechanism.

## Context And Carrier Compatibility

Shared declarations do not make every implementation portable. Native runtime
handlers use native runtime context; guest handlers use the guest's supported
context and dependency surface. Before reusing a handler catalog, verify its
context and resource bounds for the intended carrier. A native runtime test
cannot establish that a guest builds or that its imports are available.

Use actual guest compilation when target compatibility is the question. Use a
real component or sidecar invocation when testing carrier admission, message
encoding, streaming, process isolation or failure propagation. Metadata and WIT
inspection can prove a declared surface without executing the artifact; they
do not prove execution or cleanup. Only exercise carriers the product ships.

Portable call-context values need an end-to-end check across each promised
carrier when their propagation affects behavior. Native resource handles and
process-local context values must not be assumed to cross that boundary.
Include refusal or unavailable-dependency cases when they are part of the
consumer's supported contract.

## Lifecycle And Settlement

The composition owner starts shared resources and settles them on failed startup
and shutdown. Place failures at known synchronization points and observe that
partially initialized resources are released. Test failure before readiness
separately from a crash after readiness; one is not evidence for the other.

Await operation or runtime settlement before checking final state or reusing a
resource. Dropping a request instance requests cancellation; it does not prove
cleanup has completed. Resident handles hold leases: releasing the last lease
stops the instance, while explicit cancellation stops it for all holders. Select
the lifecycle action matching the behavior under test and await its result.

When reload or recovery is a product feature, prove withdrawal of the old
provider and successful use of the new generation. Keep failure and recovery
checks tied to observable discovery, dispatch and resource ownership.

## Persistence And External Resources

Test application-owned persistence through its public write, read and reopen
boundaries. Runtime tests cover operation-to-storage composition and recovery
that the product promises. They need not duplicate Harmos storage-engine tests.
Inject file or remote-service failures at a public backend/client boundary so
the real parsing, rollback and ownership behavior relevant to the test remains
involved.

Use isolated temporary storage and controlled external endpoints. Cover budget,
admission, concurrency and response-batch limits selected by the consumer where
they affect product behavior; Harmos protocol constants and correctness
constraints remain Harmos-owned. Generated artifacts require compilation tests
when buildability is the claim, while inspection alone is sufficient for inert
metadata claims.
