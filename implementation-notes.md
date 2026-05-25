# Implementation Notes — Task 6: Parse Visual Nodes, Text Content, Audio, And Resources

## Decisions Not in Spec

### roxmltree NodeType Differences
The spec referenced `NodeType::Cdata` and `NodeType::EntityReference`, but roxmltree 0.21.x merges CDATA sections and resolved entity references into `NodeType::Text` nodes. The `<text>` content handler only needs to match `NodeType::Text` — XML entity references like `&amp;` and CDATA content are both included in text nodes automatically. This is why the test `Open&amp;Cat<![CDATA[!]]>` produces `OpenCat!` as expected.

### `&str` vs `String` for `required_attr`
The spec showed `id.clone()` in several places, but `required_attr` returns `&str` (borrowed from the roxmltree document), not `String`. Changed all `.clone()` calls on `id` to `.to_string()` where the field expects `String`.

### `queryCount`/`aspectRatio` Validation
The spec's `IMAGE_ATTRS` whitelist includes `queryCount` and `aspectRatio`, meaning `ensure_allowed_attrs` alone won't reject them. Added explicit checks in `parse_image_source` that `queryCount` requires `query` and `aspectRatio` requires `query`, matching the test expectation.

### `ParentContext` Enum
Defined but only used as a parameter — currently no branching logic depends on it. It's reserved for future validation (e.g., restricting which elements can appear in certain contexts).

### `parse_transition_node` Stub
Only validates `from`, `to`, `effect`, `duration` are present and no children. Full implementation deferred to Task 7.

### Audio Source Resolution
Top-level `<audio>` elements get `AudioAttachment::Timeline`, audio inside a `<tl>` gets `AudioAttachment::Scene { scene_id }`, and audio inside other visual elements (div, etc.) gets `AudioAttachment::Timeline` as fallback.

## Tradeoffs
- Attribute whitelists are static `&[&str]` constants rather than computed sets — simple and zero-cost
- `parse_optional_u32_attr` duplicates logic from `parse_positive_i32` but for `u32` — kept separate for type clarity
