# atelier-css-4

**Status:** DONE

Retargeted the theme toggle and mermaid re-init hook in `crates/mdview/assets/app.js`
from `data-theme` to `data-scheme`, per D3/D4. `localStorage` key `mdview-theme`,
OS-preference first-load default, and the light<->dark flip are unchanged; no other
app.js behavior touched.

**Files touched:** `crates/mdview/assets/app.js`

**Full trace/evidence:** `.bee/cells/atelier-css-4.json`
