# SDK Conformance Delta

## ADDED Requirements

### Requirement: Caller-Owned Controls on Generated Roots

Every generated resource root SHALL accept the caller's metadata/credentials,
deadline override and cancellation for one call. C++ methods SHALL take the
caller's `grpc::ClientContext&`, including Watch methods whose readers borrow
that context until completion. No root SHALL replace it with an inaccessible
per-operation context. Rust requests, Python call options and C# CallOptions
SHALL preserve the same per-call concept. All method forwarding SHALL be generated.

#### Scenario: Metadata, Deadline and Cancellation on One Call

- **WHEN** a caller supplies metadata and a deadline override, observes a generated Watch opening and then cancels that call
- **THEN** the metadata and override reach its transport and the server stream settles under caller cancellation in all four SDKs

#### Scenario: Independent Following Call

- **WHEN** the caller starts another resource call
- **THEN** that call uses its own options rather than inheriting the previous call's metadata, deadline or cancellation
