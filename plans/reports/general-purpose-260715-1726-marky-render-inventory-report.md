# Marky Markdown Rendering Pipeline Inventory

## src/lib/markdown.ts

**Purpose:** Configures a single `markdown-it` instance with plugins and custom renderers, exports `renderMarkdown()` and `extractHeadings()`, orchestrates HTML sanitization.

**Key mechanisms:**

- **markdown-it instantiation** (lines 14–19): `html: true`, `linkify: true`, `typographer: true`, `breaks: false`.
- **Plugins wired** (lines 21–46):
  - `markdown-it-anchor`: Permalink inside headers, symbol "#", placement "before", custom slugify lowercase+trim+strip non-alphanumeric+collapse whitespace to dashes (lines 21–34).
  - `markdown-it-footnote`: No config (line 35).
  - `markdown-it-task-lists`: `enabled: true, label: false` (line 36).
  - `markdown-it-front-matter`: Intentionally empty handler to strip YAML blocks at document start without rendering (lines 44–46).

- **Custom core rule: source-map-attrs** (lines 50–62): Pushes `data-source-map="startLine,endLine"` onto opening tags of block-level tokens (p, h1–h6, ul, ol, li, blockquote, table, etc.) and self-closing tokens (hr, code_block). Fence tokens handled separately in fence renderer. These attributes enable the copy-as-markdown handler to map DOM selections back to source lines.

- **Custom fence renderer** (lines 65–80): Detects language info "mermaid" and wraps as `<pre class="mermaid-pending"><code>escaped-content</code></pre>` (line 73); otherwise delegates to default fence renderer with source-map attribute injected if present.

- **Custom link_open renderer** (lines 83–92): Detects `http(s)://` links and sets `target="_blank"` + `rel="noreferrer noopener"` for external links only.

- **Sanitization** (lines 96–99): `DOMPurify.sanitize()` is the LAST step after all markdown-it rendering completes. Allowlist: `target`, `class`, `id`, `aria-hidden`, `data-source-map` attributes; add `<section>` tag. This happens AFTER plugins and renderers have run, so plugins can inject markup freely and only the final HTML is sanitized.

- **Exports:** `renderMarkdown(source)` returns sanitized HTML string; `extractHeadings(source)` parses markdown-it tokens, walks heading_open → inline pairs, extracts level/text/slug from attributes (or reconstructs slug if missing).

**Non-obvious details:**

- Front-matter stripping is parse-time (before rendering), not a regex pre-filter. The intentionally empty plugin callback discards the parsed YAML block without rendering it.
- Source-map attributes are injected via token manipulation in a core rule, meaning they survive both plugin processing and default renderers.
- Mermaid blocks are marked as "pending" (not rendered at parse time), leaving them for client-side async rendering.
- Sanitization order: markdown-it plugins and renderers first (output is intermediate HTML with all markup), then DOMPurify. No double-sanitization.

---

## src/lib/highlight.ts

**Purpose:** Lazy singleton shiki highlighter with pre-loaded common language grammars and theme switching.

**Key mechanisms:**

- **Lazy loading** (lines 34–41): `getHighlighter()` caches promise; first call triggers `createHighlighter({ themes: THEMES, langs: COMMON_LANGS })` (line 38), subsequent calls return cached promise.
- **Bundled languages** (lines 8–30): TS, TSX, JS, JSX, JSON, Rust, Python, Go, Bash, Shell, YAML, TOML, HTML, CSS, SQL, Markdown, Diff, Java, C, C++, Ruby. These are pre-loaded at init time; dynamic language loading available at line 53–57.
- **Bundled themes** (line 32): `github-light`, `github-dark` only.
- **Dynamic language loading** (lines 49–57): If requested language is not loaded, attempts to load it; if load fails, falls back to `"text"`.
- **Theme selection** (lines 59–62): Maps `theme` parameter (`"light"` or `"dark"`) to shiki theme name (`github-light` or `github-dark`).
- **Export:** `highlightCode(code, lang?, theme?)` returns Promise<string> (HTML from shiki).

**Non-obvious details:**

- Themes are bundled at createHighlighter time (line 38). Both themes are always in memory; switching theme between renders does not reload the highlighter (theme is passed per-call to `codeToHtml`).
- Languages are lazy-loaded on demand if not in the common set. Load failures silently degrade to `"text"` language.
- No theme re-initialization on theme toggle — only the call-time parameter changes. This avoids recreating the highlighter.

---

## src/lib/mermaid.ts

**Purpose:** Client-side lazy async renderer for mermaid diagrams marked as `<pre class="mermaid-pending">` by the markdown parser.

**Key mechanisms:**

- **Lazy loading** (lines 4–9): `loadMermaid()` caches the promise returned by dynamic import; first call fetches module, subsequent calls return cached promise.
- **Initialization** (lines 15–20): On first `renderMermaidBlocks()` call, `mermaid.initialize()` is invoked with `startOnLoad: false` + theme config; subsequent calls also re-initialize (lines 18–19) to ensure theme is updated if it changed. **Note:** Both branches call `initialize()`, so theme changes are picked up on every call.
- **Render loop** (lines 22–38): Iterates over all `<pre class="mermaid-pending">` elements:
  - Extracts source from element text content.
  - Generates unique ID as `mermaid-${timestamp}-${counter}` (line 25).
  - Calls `mermaid.render(id, source)` to get SVG (line 29).
  - Wraps SVG in a `<div class="mermaid-block">`.
  - Preserves `data-source-map` attribute from original `<pre>` to wrapper (lines 35–36).
  - Replaces `<pre class="mermaid-pending">` with the wrapper (line 37).
  - Catches render errors and displays error message in the wrapper (line 32).

**Non-obvious details:**

- Mermaid is initialized twice on theme change: once with the old theme (if already initialized), once with the new theme (line 19). This ensures diagrams re-render with the correct theme on the next call to `renderMermaidBlocks()`.
- The ID for mermaid.render() includes a timestamp, but IDs are unique only within the current call (not across multiple renders). This is safe because the ID is only used during rendering, and mermaid does not store state keyed by ID after render completes.
- Source-map attribute is preserved through the mermaid replacement, so copy-as-markdown can still map mermaid blocks back to source.

---

## src/lib/copyAsMarkdown.ts

**Purpose:** Intercepts copy events on the rendered markdown article and writes the original source markdown lines (not rendered HTML) to the clipboard.

**Key mechanisms:**

- **Source mapping** (lines 7–16): `findMappedAncestor(node, container)` walks up the DOM from the selection anchor/focus until it finds an ancestor with `data-source-map` attribute or reaches the container root. Returns the mapped element or null.
- **Parse map** (lines 18–22): Extracts `"startLine,endLine"` from `data-source-map` attribute and returns as tuple `[startLine, endLine]`.
- **Range extraction** (lines 28–65): `getSourceRange(selection, container)` finds the leftmost and rightmost mapped elements in the selection, then walks all mapped elements in the container to find the min start line and max end line across the selection span. Handles backward selection by swapping indices (line 53). Returns `[minStart, maxEnd]` or null.
- **Line extraction** (lines 68–71): `extractLines(source, start, end)` splits source by newlines and returns `lines.slice(start, end).join("\n")`.
- **Copy handler** (lines 77–94): `handleCopyAsMarkdown(event, container, source)` calls getSourceRange, extractLines, and writes to `event.clipboardData` with `text/plain`. Prevents default clipboard behavior (`event.preventDefault()`). Returns true if handled, false to fall through.

**Non-obvious details:**

- Relies entirely on the `data-source-map` attributes injected by the markdown parser. If an element has no ancestor with the attribute, the copy handler returns null and the event is not intercepted.
- The mapping is done at parse time (markdown.ts), not at render time, so source lines are always stable relative to the original input.
- Backward selections are normalized (line 53) so start/end indices always increase.
- The copy handler is installed as a listener on the article element (Viewer.tsx line 101), not globally. Only markdown content can trigger it.

---

## src/components/Viewer.tsx

**Purpose:** React component that orchestrates the full markdown render pipeline: markdown → HTML, syntax highlighting, mermaid rendering, copy-button attachment, copy-as-markdown interception, and scroll position memory.

**Render/sanitize/load order:**

1. **HTML generation** (line 36): `renderMarkdown(source)` → sanitized HTML string. Effect dep: `[source]`.
2. **Shiki highlighting** (lines 44–68): Async effect walks all `<pre> > code[class*='language-']` elements, calls `highlightCode()` per block, replaces `<pre>` with Shiki's `<pre class="shiki">` output. Preserves `data-source-map` if present (lines 61–62). Effect dep: `[html, resolved]`.
3. **Copy-button attachment** (line 70): `attachCopyButtons()` adds "Copy" button to all `<pre>` that don't already have one (and excludes mermaid-pending blocks).
4. **Mermaid rendering** (line 71): `renderMermaidBlocks()` replaces `<pre class="mermaid-pending">` with rendered SVG. Awaits mermaid module load and render.
5. **Callback** (line 72): Calls `onRendered()` after highlighting and mermaid complete.
6. **Copy-as-markdown listener** (line 101): Installed after HTML injection; intercepts `copy` events.

**Key mechanisms:**

- **Effect cancellation** (lines 42–77): Async effect sets `cancelled` flag on unmount; all awaits check it and return early if true. Prevents dangling state updates if component unmounts mid-render.
- **Theme tracking** (line 23): `const { resolved } = useTheme()` is a dep of the highlight effect; theme changes trigger re-highlighting.
- **Memoized innerHTML** (line 110): `dangerousHtml` is a `React.useMemo(() => ({ __html: html }), [html])`. This prevents React from re-applying `dangerouslySetInnerHTML` on every parent re-render (e.g., when parent bumps a nonce), which would wipe the shiki spans and restore the raw markdown-it `<pre>`.
- **onRendered memoization** (lines 30–33): `onRenderedRef` is a ref that always holds the current callback, but is NOT a dep of the async effect. This avoids retriggers when parent re-renders with a new onRendered function.
- **Scroll memory** (lines 17, 85–89): Map keyed by filePath; saves/restores scroll position on remount.
- **Copy-as-markdown conditional** (line 95): Installed only if `copyAsMarkdown` preference is true.
- **Effect deps** (lines 79, 90, 103): Carefully scoped to avoid unnecessary re-runs. `onRendered` and `ref` are intentionally omitted from the highlight effect because they are either stable or accessed via a ref.

**Non-obvious details:**

- HTML is set on line 36 in one effect; highlighting is done on lines 44–68 in a separate effect. Both are sequential in the same frame initially, but if source changes, the second effect only runs after the first completes. This two-effect pattern allows React to batch updates.
- Shiki replaces `<pre>` with a template-parsed result (lines 56–64), which means the replacement is a fresh DOM node. Source-map attribute is manually preserved (line 62).
- The copy-button effect (line 70) runs INSIDE the async effect, so it always runs after highlighting completes but before the `onRendered` callback.
- The `cancelled` flag is checked after every `await` to ensure we don't hydrate a stale component. This is critical because highlighting and mermaid rendering are async.

---

## src/components/CodeCopyOverlay.tsx

**Purpose:** Attaches a "Copy" button to every `<pre>` block in the rendered markdown (except mermaid blocks), handling copy-to-clipboard with transient "Copied!" feedback.

**Key mechanisms:**

- **Button attachment** (lines 8–26): Queries all `<pre>` elements in the root. For each, checks if it already has a `.copy-code-btn` (line 11) and skips if so. Skips mermaid-pending blocks (line 12). Creates a button element with `class="copy-code-btn"`.
- **Copy handler** (lines 17–23): On click, extracts text from `<pre> > <code>` or falls back to `<pre>` text content (line 18). Writes to `navigator.clipboard`. On success, sets button text to "Copied!" for 1200ms, then restores "Copy".
- **Direct DOM manipulation** (line 24): Appends button directly to `<pre>` without React. This avoids rebuilding the markdown as React nodes, which would be expensive.

**Non-obvious details:**

- Button is re-used (idempotent check on line 11) if it already exists. This allows the function to be called multiple times safely (e.g., if new code blocks are added dynamically, which is unlikely in this app but safe by design).
- Mermaid blocks are explicitly excluded (line 12) because they are not code blocks and should not have a copy button.
- The component exports a dummy React component (line 28) for no reason — the actual work is the exported function `attachCopyButtons()`. This suggests the component was originally meant to be a React component but was refactored to direct DOM manipulation.

---

## src/styles/markdown.css

**Purpose:** CSS prose styling for the rendered markdown article, using Tailwind @apply and CSS variables from shadcn/ui for theming.

**Structural approach:**

- **Base container** (lines 3–8): `.markdown-body` is a centered, max-width prose container with padding and text color/line-height from Tailwind utilities. `@reference "./index.css"` imports global styles (line 1).
- **Margin collapse** (lines 11–12): Removes top/bottom margins on first/last children to avoid double spacing.
- **Headings** (lines 14–30): H1–H6 with margin, scroll offset, border-bottom (h1, h2), ascending font sizes, and muted colors for h5/h6. Margins and borders use CSS variables (`var(--border)`).
- **Paragraphs, links, emphasis** (lines 32–38): Margins, link color from `var(--primary)`, hover opacity, strong/em/strikethrough styling using Tailwind utilities and CSS variables.
- **Lists** (lines 40–48): Margin, padding, list style (disc, decimal), item margins. Nested lists have reduced margins.
- **Task lists** (lines 50–55): Checkboxes use `accent-color: var(--primary)`, list-style set to none, left margin adjusted.
- **Blockquotes** (lines 57–62): Muted text, left border, padding, margin using CSS variables.
- **Tables** (lines 70–84): Full-width, bordered, overflow-x scroll. Header rows have muted background; alternating row backgrounds use `color-mix(in oklch, ...)` to blend with muted.
- **Inline and block code** (lines 86–114): Inline code (`code:not(pre code)`) has background, rounded corners, monospace font, small padding. `<pre>` has relative position (for copy button positioning), border, overflow, background. Shiki `<pre class="shiki">` gets matching padding. All code-related rules reset backgrounds to transparent when code is highlighted by Shiki.
- **Copy button** (lines 117–132): Positioned absolute top-right of `<pre>`, opacity 0 by default, appears on pre:hover. Small font, semi-transparent background using `color-mix()`, muted text color.
- **Images** (line 134): Max-width 100%, rounded corners.
- **Mermaid blocks** (lines 136–145): Styled as bordered boxes with background, padding, center alignment, SVG constrained to max-width.
- **Search highlights** (lines 147–157): `.doc-search-match` and `.doc-search-match.doc-search-active` for document search feature (not shown in inventory scope, but styled here).
- **Footnotes** (lines 159–165): Muted text, top border, top padding, smaller font.

**Notable rules:**

- All color values defer to CSS variables: `var(--background)`, `var(--foreground)`, `var(--muted)`, `var(--border)`, `var(--primary)`, `var(--primary-foreground)`, `var(--muted-foreground)`. This enables light/dark theme switching without recompiling CSS.
- `color-mix(in oklch, ...)` is used for blended colors (e.g., alternating table row backgrounds, semi-transparent button background). This is a CSS-only approach to color mixing without needing SCSS or PostCSS plugins.
- `scroll-margin-top: 4rem` on headings (line 23) prevents them from scrolling behind a sticky header (not shown in inventory scope).
- Copy button uses `opacity` transition (line 129) for smooth appearance on hover.
- Monospace fonts are listed in order of preference: `ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace` (lines 88, 105). This is system font stacking.
- `pre.shiki` (from Shiki) receives padding directly; the code inside it has `background: transparent; padding: 0` to avoid double padding (lines 114–115).

**CSS variables and theme coupling:**

- All structural colors and some spacing depend on shadcn CSS variables. Light/dark theme is switched by toggling the `dark` class on `<html>` (shadcn convention), which redefines the variable values.
- Markdown styling does not hard-code light/dark mode — it always reads from variables, so it is theme-agnostic.

---

## src/lib/markdown.test.ts

**Purpose:** Vitest snapshot and feature tests for `renderMarkdown()` and `extractHeadings()`.

**Test coverage:**

- Anchors: Headings get auto-generated IDs with slugified names (test lines 5–10).
- Tables: GFM-style tables are rendered (test lines 12–18).
- Task lists: Checkboxes with checked state (test lines 20–25).
- Code blocks: Language class attached to code element (test lines 27–31).
- Mermaid: Blocks are marked `mermaid-pending` for client-side rendering (test lines 33–38).
- External links: `target="_blank"` and `rel="noreferrer noopener"` applied (test lines 40–44).
- Relative links: No `target="_blank"` on relative URLs (test lines 46–49).
- Sanitization: Script tags are removed, text is preserved (test lines 51–55).
- Strikethrough: `~~text~~` renders as `<s>text</s>` (test lines 57–60).
- Blockquotes: `<blockquote>` tags are created (test lines 62–65).
- Front matter: YAML at document start is stripped, content below is rendered (test lines 67–75).
- Thematic breaks: Standalone `---` (not front matter) renders as `<hr>` (test lines 77–83).
- Mid-document YAML blocks: Not stripped, rendered as content (test lines 85–93).
- Heading extraction: `extractHeadings()` returns level/text/slug for each heading (test lines 97–104).
- No headings: Returns empty array when no headings exist (test lines 106–109).

No tests for individual plugin behavior (e.g., footnote rendering, anchor link structure) beyond the headline features. Tests focus on high-level rendering correctness and sanitization.

---

## Summary of Files Not Read

All files in scope were read successfully.

---

Status: DONE | Summary: Marky's markdown rendering pipeline orchestrates markdown-it + plugins → DOMPurify sanitization → async Shiki highlighting + mermaid rendering, with source-map attributes enabling copy-as-markdown and theme-coupled rendering via CSS variables and shiki/mermaid re-initialization.
