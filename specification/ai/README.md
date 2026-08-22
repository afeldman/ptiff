# PTIFF Specification — AI / ML Domain

The AI domain represents AI / ML-derived products (segmentation, classification, uncertainty,
embeddings) and their provenance. It is currently **conceptual / reserved** (no dedicated
private tag); per-pixel AI layers are carried via the Scientific-Layers tag (65004,
`ptiff.layers.*`) and product provenance via the Provenance tag (65005, `ptiff.provenance.*`).

## Documents

| Document | Contents |
|----------|----------|
| [`ai-products.md`](./ai-products.md) | Purpose, current implementation, planned scope |

## Related

- [`../core/container-encoding.md`](../core/container-encoding.md) — tags 65004/65005 record rules.
- [`../appendices/field-register.md`](../appendices/field-register.md) — field registration.
- `RFC-0001-Core.md` — §9 UC5, §17 item 7.
