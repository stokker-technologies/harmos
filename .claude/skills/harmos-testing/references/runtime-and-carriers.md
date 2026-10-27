<!-- Rendered by stkr project update from .agents/skills/harmos-testing/references/runtime-and-carriers.md; sha256:1c1ba02bad27e6324d9752b4cdc1186fcbd6885869b2ee7f87be94b0271235c9 -->

# Runtime And Carriers

## Runtime Composition

Exercise the product's public runtime construction and operation dispatch when
testing composition. Initialize a shared resource once and verify that the
handlers using it observe the same state. Keep resource-specific behavior in
the resource library's tests; use controlled substitutes at that library's
public boundary when a runtime test needs a particular failure.

For a native plugin composing a reusable product, consume the entry facade with
its `runtime` feature enabled. Check that the advertised catalog contains the
original operation declarations and that its runtime registration supplies the
intended factories and dependency snapshot. Declaration discovery alone does not prove that a
handler is bound or dispatchable. Exercise a representative call through the
composed plugin and observe its resource effects.

Keep default domain-only facade usage independent of runtime resources. If the
product promises both domain-only and runtime-enabled consumers, cover both
dependency configurations with the project's existing check mechanism.

## Context And Carrier Compatibility

Native, sidecar and guest bodies implement the common SDK `Operation` trait
with `OperationContext`. Dependency snapshots carry process-local resources;
sharing a declaration does not make those resources target-compatible. Check
contract-only and runtime-enabled configurations separately, including actual
guest dependency availability. A native test cannot prove guest buildability.

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
cleanup has completed. Services use `ServicePolicy`, independent of request retry policy. Verify
failed cleanup retains ownership and that a repeated `unregister` resumes
settlement before re-registration. Verify fresh generations fence stale work.

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
