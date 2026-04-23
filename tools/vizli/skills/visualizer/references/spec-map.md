# Vizli Spec Map

Use this file to decide which authoritative repo doc to consult before
loading a larger spec.

## Read order

1. Start with the sidecar for the candidate template.
2. If selection is still ambiguous, use [`../../../SIDECAR_SPEC.md`](../../../SIDECAR_SPEC.md).
3. If the issue is command syntax or output support, use [`../../../OUTPUT_SPEC_FINAL.md`](../../../OUTPUT_SPEC_FINAL.md).
4. If the issue is payload shape, schemas, or template capabilities, use [`../../../TEMPLATE_SPEC_FINAL.md`](../../../TEMPLATE_SPEC_FINAL.md).
5. If the issue is the operator sequence itself, use [`../../../VIZLI_README.md`](../../../VIZLI_README.md).

## What each doc is for

- `VIZLI_README.md`
  The agent operating manual. Use it when you need the end-to-end runtime sequence.
- `SIDECAR_SPEC.md`
  The discovery contract. Use it when searching templates, interpreting tags, or deciding whether a sidecar is complete.
- `OUTPUT_SPEC_FINAL.md`
  The CLI contract. Use it for command flags, supported formats, `inspect`, `validate`, `golden`, and `doctor`.
- `TEMPLATE_SPEC_FINAL.md`
  The payload and template contract. Use it for `params`, `datasets`, `meta`, directory packages, and schema-driven behavior.

## Practical rule

Do not read whole specs by default. Load the smallest authoritative file that
answers the current question.
