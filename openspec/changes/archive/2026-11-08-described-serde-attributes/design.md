## Agreed Design

The schema graph retains explicit per-definition Serde wire representation metadata. Ordinary object schemas retain their existing canonical bytes. The canonical identity function owns encoding for revisions and fingerprints, including wire metadata. Naming uses serialization names; deserialization-only attributes do not change the stored shape. Unsupported custom serialization and flattening fail explicitly rather than guessing their output.

The derive is the only authoring owner. Enum payload styles, tagging keys and omission rules remain distinguishable from ordinary fields and nullability. Stored schemas preserve that metadata across declaration markers and graph serialization. Existing portable message schemas continue to use their existing representation. Transparent structs forward their inner field kind and cardinality, so adding or removing a wrapper preserves identity. Default field-one scalar value metadata does not change the existing canonical vectors. Omission has its own presence wrapper and can distinguish an absent option from a present null-valued payload.

## Validation Boundary

Use targeted macro/domain tests while implementing. Then build the CI workspace test archive and run lint:rust once, with CARGO_BUILD_JOBS=8, lowering on allocation failure. The owner explicitly excludes full lanes and simulations. Run the supplied scan before merging. No worktrees or other writers.
