---
artifact_contract: bee-plan/v1
artifact_readiness: implementation-ready
mode: small
---

# Polish settings form (PBI-15)

The settings form already had Atelier `.fg-field`/`.fg-input`/`.fg-select` markup
(from PBI-08) but no glue for the form container, fieldsets, or legends — so it
rendered as default browser fieldsets ("xấu & tùm lum"). 1 flag
(existing-covered-behavior), 2 files → small.

## Cell

`polish-settings-form-1` — add `class="fg-settings"` to the settings `<form>`
(`views.rs`) and app.css glue (Atelier tokens only): fieldsets as surface cards
(hairline border + `--radius-lg`), legends as uppercase eyebrows, spaced
`.fg-field`/`.fg-check`, left-aligned submit. No functional change.

## Verification

`cargo test --workspace` green; served `/settings` HTML carries `.fg-settings`;
served CSS carries the glue. Confirmed live.

No overlap with the concurrent host_name/port session (UI files only).
