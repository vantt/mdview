# Acme Payments

Documentation lives in `docs/`.

## Plans

Plans in `docs/plans/` follow the house template, in this order:

1. `# <Title> plan`, then one paragraph that links the ADRs it builds on.
2. `## Goals`: three bullets.
3. `## Retry schedule` (or the topic's equivalent): a table.
4. `## Flow`: one Mermaid `sequenceDiagram`.
5. `## Config`: one YAML block that sets `max_retries`.
6. `## Steps`: a numbered list.

Keep plans under 80 lines.

Keep replies about a plan to the file path (and its link, if you have one) plus one short clause on what it covers; do not mention this template.
