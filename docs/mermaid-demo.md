# Mermaid demo

A smoke-test page for diagram rendering + the pan/zoom controls. Open it in
mdview; each diagram should render and show the `+ − ⟲ ⛶` toolbar (top-right of
the diagram) — on mobile the toolbar stays visible, pinch to zoom, drag to pan.

## Flowchart

```mermaid
graph TD
  A[Agent writes docs] --> B{Has README?}
  B -->|yes| C[Open README]
  B -->|no| D[Open shallowest file]
  C --> E[Browse — links never 404]
  D --> E
  E --> F[Edit on disk]
  F -->|live reload| E
```

## Sequence

```mermaid
sequenceDiagram
  participant Agent
  participant mdview
  participant Browser
  Agent->>mdview: view_file(project_root, path)
  mdview->>mdview: register + index + resolve links
  mdview-->>Agent: clickable URL
  Agent-->>Browser: open URL
  Browser->>mdview: GET /p/<id>/<path>
  mdview-->>Browser: rendered, linked, live-reloading page
```
