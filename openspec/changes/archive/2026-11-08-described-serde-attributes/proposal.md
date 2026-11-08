## Why

Dokime PR #368 cannot adopt Harmos shape identities because its stored Serde types use tagging, naming and omission attributes that Described rejects.

## What Changes

- Describe Serde external, internal, adjacent and untagged enums, names, defaults, skips and the Option::is_none and Vec::is_empty omission predicates used by Dokime.
- Retain wire representation in the schema and its single canonical identity encoding. Preserve existing golden identities.
- Refuse unsupported representations at compile time naming the attribute and type. Verify supported forms against structurally decoded Serde values.

## Impact

The domain schema, derive macro, schema codecs, macro fixtures and product guide own this change. Dokime remains read-only. The owner authorizes ready PR delivery and immediate squash to dev after the specified diff scanner reports CLEAN.
