# Wire Format

Status: implemented wire definition; remaining owner confirmations are listed in `../design.md`.
`harmos/v1/plugin.proto` mirrors the landed definition at repository-root `proto/harmos/v1/plugin.proto`.
The runtime behaviour and fingerprint rules are in repository-root `proto/harmos/v1/behaviour.md`.

## Compatibility Rules

These hold for every release of `harmos.v1` once the first one ships.

- Changes are additive only: new messages, new fields, new enum values and
  new calls. A field's number, name and type never change.
- A removed field or enum value is `reserved` by number and name and never
  reused.
- Both sides ignore fields and enum values they do not know, so an older
  plugin works with a newer host and the reverse.
- Every enum has an unspecified zero value, and a receiver treats an unknown
  value as unspecified.
- `revision` in `DescribeRequest` and `Description` says how far each side
  goes. A side uses a feature only when the other's revision includes it.
- A change that cannot follow these rules is `harmos.v2`, a new package
  served beside `v1` for one release.
- Operation payloads travel as bytes with a fully qualified type name. The
  host routes them and never decodes them.
- An operation failure is a `Failure` message. gRPC status codes report only
  transport faults.

## Wire Contracts

| Decision | Reason |
| --- | --- |
| Operations are unary: one request, one response | Streaming operations are not needed by a plugin once samples have `Publish`; large uploads go as a sequence of unary calls |
| No deadline on the wire | The host enforces the policy and cancels, as today |
| `Samples` wraps a repeated `Sample` | One sample per message now; grouping later needs no protocol change |
| `Sample.sequence` | Lets the host record a gap after a dropped connection |
| `Section.type` is a message name, with no separate schema number | The message definition is the schema |
| A child call is request and response, with `ChildCancel` | Replaces submit, join, status, release and detach handles |
| One lifecycle call, `Stop` | Settings are read through a host operation, so nothing is needed at start; `Stop` lets a plugin release hardware before the host closes |
| No encryption or credentials on a plugin endpoint | The network is trusted, and a server written by hand from these files must work |
| `Failure.action` | Every failure names what the user can do |
| Two visibility values | Plugins and clients are all trusted, so `Private` (the plugin and the host) and `Public` (everyone connected) are enough |
| The host owns the clock: `Tick` on `Publish`, and `harmos.clock.sleep` | One clock for the host and every plugin gives one time base for evidence and lets a test advance time for a plugin in any language |

## Retry Policy

A retry carries its delay as `OperationPolicy.backoff`. An absent policy has no deadline, zero
cancellation grace and no retries. These defaults await confirmation in the owning design.
