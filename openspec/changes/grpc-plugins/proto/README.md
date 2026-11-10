# Wire Format Draft

Status: draft for owner review on 2026-11-10; not agreed and not implemented.
`harmos/v1/plugin.proto` is the complete proposed `harmos.v1`.

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

## Decisions Taken in the Draft

Each is open to review.

| Decision | Reason |
| --- | --- |
| Operations are unary: one request, one response | Streaming operations are not needed by a plugin once samples have `Publish`; large uploads go as a sequence of unary calls |
| No deadline on the wire | The host enforces the policy and cancels, as today |
| `Samples` wraps a repeated `Sample` | One sample per message now; grouping later needs no protocol change |
| `Sample.sequence` | Lets the host record a gap after a dropped connection |
| `Section.type` is a message name, with no separate schema number | The message definition is the schema |
| A child call is request and response, with `ChildCancel` | Replaces submit, join, status, release and detach handles |
| No lifecycle call | Settings are read through a host operation, and a managed plugin stops with its process |
| `Failure.action` | Every failure names what the user can do |

## Questions for Review

1. `Visibility` keeps today's three values. If connected plugins and clients
   are all trusted, two would do: `PRIVATE` (the plugin and the host) and
   `PUBLIC` (everyone connected).
2. Whether a plugin needs a call when the host is about to disconnect, to
   release hardware in an orderly way outside any execution.
3. Whether `OperationPolicy.attempts` is enough, or retries need a delay
   between attempts on the wire.
