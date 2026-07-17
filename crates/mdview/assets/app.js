// Theme toggle (cycles light → dark) with persistence, and WebSocket live reload.
(function () {
  "use strict";

  function applyTheme(t) {
    var dark = t === "dark" || (t === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.setAttribute("data-scheme", dark ? "dark" : "light");
  }

  var toggle = document.getElementById("theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var cur = document.documentElement.getAttribute("data-scheme");
      var next = cur === "dark" ? "light" : "dark";
      try { localStorage.setItem("mdview-theme", next); } catch (e) {}
      applyTheme(next);
      // Re-render mermaid diagrams for the new theme, if present.
      if (window.__mermaid) {
        try {
          window.__mermaid.initialize({ startOnLoad: false, theme: next === "dark" ? "dark" : "default" });
        } catch (e) {}
      }
    });
  }

  // Chapter sidebar (C2 breadcrumb-zoom): always show exactly one folder —
  // its subfolders (zoom in) and its files by title — with a clickable
  // breadcrumb to zoom out. Default focus = the current file's folder.
  (function () {
    var root = document.getElementById("chapter");
    var data = document.getElementById("filelist");
    if (!root || !data) return;

    var files;
    try { files = JSON.parse(data.textContent || "[]"); } catch (e) { return; }
    var pid = root.getAttribute("data-pid") || "";
    var rootLabel = root.getAttribute("data-root") || "/";
    var current = root.getAttribute("data-current") || "";

    function dirOf(p) { var i = p.lastIndexOf("/"); return i < 0 ? "" : p.slice(0, i); }
    function baseOf(p) { var i = p.lastIndexOf("/"); return i < 0 ? p : p.slice(i + 1); }
    function el(tag, cls, text) {
      var e = document.createElement(tag);
      if (cls) e.className = cls;
      if (text != null) e.textContent = text;
      return e;
    }

    var focus = dirOf(current); // start in the current file's folder

    // Whether the subfolders disclosure is expanded — remembered for the
    // session (auto-opens when a folder has no files of its own, see below).
    var foldersOpen = false;
    try { foldersOpen = sessionStorage.getItem("mdview-folders-open") === "1"; } catch (e) {}

    function render() {
      root.textContent = "";

      // Breadcrumb: root + each ancestor segment, all clickable to zoom out.
      var bc = el("div", "chap-crumbs");
      var rootSeg = el("button", "chap-seg", rootLabel);
      rootSeg.addEventListener("click", function () { focus = ""; render(); });
      bc.appendChild(rootSeg);
      if (focus) {
        var segs = focus.split("/");
        var acc = "";
        segs.forEach(function (s) {
          acc = acc ? acc + "/" + s : s;
          var path = acc;
          bc.appendChild(el("span", "chap-sep", "›"));
          var b = el("button", "chap-seg", s);
          b.addEventListener("click", function () { focus = path; render(); });
          bc.appendChild(b);
        });
      }
      root.appendChild(bc);

      // Partition the focus folder into immediate subfolders and direct files.
      var prefix = focus ? focus + "/" : "";
      var folders = {};
      var here = [];
      files.forEach(function (f) {
        if (focus && f.p.indexOf(prefix) !== 0) return;
        var rest = focus ? f.p.slice(prefix.length) : f.p;
        var slash = rest.indexOf("/");
        if (slash < 0) here.push(f);
        else folders[rest.slice(0, slash)] = true;
      });
      var folderNames = Object.keys(folders).sort();

      // Every subfolder collapses into ONE disclosure bar, so however many there
      // are they never crowd out the chapter list. Collapsed by default; opens
      // automatically when this folder has no files (else it would look empty).
      if (folderNames.length) {
        var open = foldersOpen || here.length === 0;
        var box = el("div", "chap-folders" + (open ? " is-open" : ""));

        var bar = el("button", "chap-folders__bar");
        bar.setAttribute("aria-expanded", open ? "true" : "false");
        bar.appendChild(el("span", "chap-folders__chev", "›"));
        bar.appendChild(el("span", "chap-folders__label", "Subfolders"));
        bar.appendChild(el("span", "chap-folders__count", String(folderNames.length)));
        bar.addEventListener("click", function () {
          foldersOpen = !box.classList.contains("is-open");
          try { sessionStorage.setItem("mdview-folders-open", foldersOpen ? "1" : "0"); } catch (e) {}
          box.classList.toggle("is-open", foldersOpen);
          bar.setAttribute("aria-expanded", foldersOpen ? "true" : "false");
        });
        box.appendChild(bar);

        var list = el("div", "chap-folders__list");
        var inner = el("div", "chap-folders__inner");
        folderNames.forEach(function (name) {
          var b = el("button", "chap-subfolder", name);
          b.addEventListener("click", function () {
            focus = focus ? focus + "/" + name : name;
            render();
          });
          inner.appendChild(b);
        });
        list.appendChild(inner);
        box.appendChild(list);
        root.appendChild(box);
      }

      // The chapter list: files in this folder, by title, current one active.
      if (here.length) {
        root.appendChild(el("div", "chap-sec", "Chapters"));
        here
          .map(function (f) { return { f: f, label: f.t && f.t.length ? f.t : baseOf(f.p) }; })
          .sort(function (a, b) { return a.label.localeCompare(b.label); })
          .forEach(function (item) {
            var a = el("a", "chap-file" + (item.f.p === current ? " active" : ""), item.label);
            a.href = "/p/" + pid + "/" + item.f.p;
            root.appendChild(a);
          });
      }
    }

    render();
  })();

  // Fuzzy file-jump palette (Cmd/Ctrl+K): fetch nucleo-ranked files from the
  // server /p/:id/_jump endpoint and navigate. Complements full-text search —
  // this jumps by file name/path, that searches content.
  (function () {
    var chapter = document.getElementById("chapter");
    var pid = chapter && chapter.getAttribute("data-pid");
    if (!pid) return;

    var overlay, input, list;
    var hits = [];
    var sel = 0;
    var seq = 0; // request sequence — drop responses that a later query superseded
    var timer = null;

    function build() {
      overlay = document.createElement("div");
      overlay.className = "jump-overlay";
      overlay.setAttribute("hidden", "");
      var box = document.createElement("div");
      box.className = "jump-box";
      input = document.createElement("input");
      input.className = "jump-input";
      input.type = "text";
      input.placeholder = "Jump to file…";
      input.setAttribute("aria-label", "Jump to file");
      list = document.createElement("ul");
      list.className = "jump-list";
      box.appendChild(input);
      box.appendChild(list);
      overlay.appendChild(box);
      document.body.appendChild(overlay);

      overlay.addEventListener("mousedown", function (e) {
        if (e.target === overlay) close();
      });
      input.addEventListener("input", onInput);
      input.addEventListener("keydown", onKey);
    }

    function isOpen() { return overlay && !overlay.hasAttribute("hidden"); }

    function open() {
      if (!overlay) build();
      overlay.removeAttribute("hidden");
      input.value = "";
      hits = [];
      sel = 0;
      render();
      input.focus();
    }

    function close() {
      if (overlay) overlay.setAttribute("hidden", "");
    }

    function onInput() {
      if (timer) clearTimeout(timer);
      timer = setTimeout(fetchHits, 120);
    }

    function fetchHits() {
      var q = input.value.trim();
      if (!q) { hits = []; sel = 0; render(); return; }
      var mine = ++seq;
      fetch("/p/" + encodeURIComponent(pid) + "/_jump?q=" + encodeURIComponent(q))
        .then(function (r) { return r.ok ? r.json() : []; })
        .then(function (data) {
          if (mine !== seq) return; // a newer keystroke already fired
          hits = Array.isArray(data) ? data : [];
          sel = 0;
          render();
        })
        .catch(function () { if (mine === seq) { hits = []; render(); } });
    }

    function render() {
      list.textContent = "";
      hits.forEach(function (h, i) {
        var li = document.createElement("li");
        li.className = "jump-item" + (i === sel ? " active" : "");
        var t = document.createElement("span");
        t.className = "jump-title";
        t.textContent = h.title && h.title.length ? h.title : h.rel_path;
        var p = document.createElement("span");
        p.className = "jump-path";
        p.textContent = h.rel_path;
        li.appendChild(t);
        li.appendChild(p);
        li.addEventListener("mousedown", function (e) { e.preventDefault(); go(i); });
        list.appendChild(li);
      });
    }

    function go(i) {
      var h = hits[i];
      if (h) window.location.href = h.url;
    }

    function onKey(e) {
      if (e.key === "Escape") { e.preventDefault(); close(); }
      else if (e.key === "ArrowDown") { e.preventDefault(); if (hits.length) { sel = (sel + 1) % hits.length; render(); } }
      else if (e.key === "ArrowUp") { e.preventDefault(); if (hits.length) { sel = (sel - 1 + hits.length) % hits.length; render(); } }
      else if (e.key === "Enter") { e.preventDefault(); go(sel); }
    }

    document.addEventListener("keydown", function (e) {
      if ((e.metaKey || e.ctrlKey) && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        if (isOpen()) close(); else open();
      }
    });
  })();

  // Copy-as-markdown: when the user copies a selection inside the rendered
  // article, substitute the RAW markdown for the selected block range (mapped
  // via data-sourcepos line numbers) instead of the rendered HTML/text.
  (function () {
    var article = document.querySelector(".markdown-body");
    var srcEl = document.getElementById("mdsource");
    if (!article || !srcEl) return;

    var source;
    try { source = JSON.parse(srcEl.textContent || '""'); } catch (e) { return; }
    if (typeof source !== "string" || !source.length) return;
    var lines = source.split("\n");

    // Parse comrak's data-sourcepos "startLine:col-endLine:col" → [start, end].
    function rangeOf(el) {
      var sp = el.getAttribute("data-sourcepos");
      if (!sp) return null;
      var m = /^(\d+):\d+-(\d+):\d+$/.exec(sp);
      if (!m) return null;
      return [parseInt(m[1], 10), parseInt(m[2], 10)];
    }

    document.addEventListener("copy", function (e) {
      var sel = window.getSelection();
      if (!sel || sel.rangeCount === 0 || sel.isCollapsed) return;

      // Only act when the selection lives inside the rendered article.
      var anchor = sel.anchorNode;
      if (!anchor || !article.contains(anchor)) return;

      // Collect the source line range across every mapped block the selection
      // touches (partial containment), then union to a single [min, max].
      var blocks = article.querySelectorAll("[data-sourcepos]");
      var min = Infinity, max = -Infinity;
      for (var i = 0; i < blocks.length; i++) {
        if (!sel.containsNode(blocks[i], true)) continue;
        var r = rangeOf(blocks[i]);
        if (!r) continue;
        if (r[0] < min) min = r[0];
        if (r[1] > max) max = r[1];
      }
      if (min === Infinity || max < min) return; // nothing mapped → default copy

      var md = lines.slice(min - 1, max).join("\n");
      if (!md) return;
      if (e.clipboardData) {
        e.clipboardData.setData("text/plain", md);
        e.preventDefault();
      }
    });
  })();

  // Project card timestamps: the server sends a raw ISO instant in
  // <time datetime>; render it in the viewer's own locale/timezone as a short
  // relative age (older than a week → an absolute date).
  (function () {
    var times = document.querySelectorAll("time.proj-card__time[datetime]");
    if (!times.length) return;
    function fmt(iso) {
      var d = new Date(iso);
      if (isNaN(d.getTime())) return null;
      var secs = (Date.now() - d.getTime()) / 1000;
      if (secs < 60) return "just now";
      if (secs < 3600) return Math.floor(secs / 60) + " min ago";
      if (secs < 86400) return Math.floor(secs / 3600) + "h ago";
      if (secs < 604800) return Math.floor(secs / 86400) + "d ago";
      return d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
    }
    times.forEach(function (t) {
      var iso = t.getAttribute("datetime");
      var s = fmt(iso);
      if (!s) return;
      t.textContent = s;
      t.title = new Date(iso).toLocaleString();
    });
  })();

  // Project delete (home page): confirm before unregistering. The form still
  // POSTs normally if scripting is off — this only guards against a stray tap.
  (function () {
    var forms = document.querySelectorAll(".proj-card__delete");
    if (!forms.length) return;
    forms.forEach(function (f) {
      f.addEventListener("submit", function (e) {
        var name = f.getAttribute("data-project") || "this project";
        var ok = window.confirm(
          "Remove “" + name + "” from mdview?\n\n" +
          "The files stay on disk — only the registry entry and its index are removed. " +
          "Re-registering re-scans them."
        );
        if (!ok) e.preventDefault();
      });
    });
  })();

  // Mobile sidebar drawer: the file-tree sidebar is hidden at narrow widths,
  // so the topbar hamburger toggles it open as an overlay (with a backdrop).
  (function () {
    var layout = document.querySelector(".layout");
    var toggle = document.getElementById("sidebar-toggle");
    if (!layout || !toggle) return;
    var backdrop = layout.querySelector(".sidebar-backdrop");
    function set(open) {
      layout.classList.toggle("sidebar-open", open);
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
    }
    toggle.addEventListener("click", function () {
      set(!layout.classList.contains("sidebar-open"));
    });
    if (backdrop) backdrop.addEventListener("click", function () { set(false); });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") set(false);
    });
    // Picking a file navigates (full reload); a folder click only zooms the
    // tree in place, so close only when an actual file link is chosen.
    var sb = layout.querySelector(".sidebar");
    if (sb) sb.addEventListener("click", function (e) {
      if (e.target.closest(".chap-file")) set(false);
    });
  })();

  // Code-block copy button. Each rendered code block (`<pre class="code">`, not
  // mermaid) gets wrapped in the design system's .fg-codeblock component with a
  // top bar carrying its language label and a Copy button. Done client-side
  // because the server's HTML sanitizer would strip a server-emitted <button>.
  (function () {
    var blocks = document.querySelectorAll(".fg-prose pre.code");
    if (!blocks.length || !document.body) return;

    function langOf(pre) {
      var code = pre.querySelector("code");
      if (!code) return "";
      var m = /(?:^|\s)language-([\w+#.-]+)/.exec(code.className || "");
      return m ? m[1] : "";
    }

    function copyText(text, btn) {
      function ok() {
        var prev = btn.textContent;
        btn.textContent = "Copied";
        btn.classList.add("is-copied");
        setTimeout(function () {
          btn.textContent = prev;
          btn.classList.remove("is-copied");
        }, 1400);
      }
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(ok, function () { fallback(text, ok); });
      } else {
        fallback(text, ok);
      }
    }
    function fallback(text, ok) {
      var ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      try { document.execCommand("copy"); ok(); } catch (e) {}
      document.body.removeChild(ta);
    }

    blocks.forEach(function (pre) {
      if (pre.parentElement && pre.parentElement.classList.contains("fg-codeblock")) return;
      var code = pre.querySelector("code");
      if (!code) return;

      var wrap = document.createElement("div");
      wrap.className = "fg-codeblock";
      var bar = document.createElement("div");
      bar.className = "fg-codeblock__bar";
      var label = document.createElement("span");
      label.className = "fg-codeblock__lang";
      label.textContent = langOf(pre) || "text";
      var btn = document.createElement("button");
      btn.type = "button";
      btn.className = "fg-codeblock__copy";
      btn.textContent = "Copy";
      btn.setAttribute("aria-label", "Copy code to clipboard");
      btn.addEventListener("click", function () { copyText(code.textContent, btn); });
      bar.appendChild(label);
      bar.appendChild(btn);

      pre.parentNode.insertBefore(wrap, pre);
      wrap.appendChild(bar);
      wrap.appendChild(pre);
    });
  })();

  // Copy the whole page's Markdown source (embedded as JSON in #mdsource) —
  // complements the selection-based copy-as-markdown above.
  (function () {
    var btn = document.getElementById("copy-md");
    var src = document.getElementById("mdsource");
    if (!btn || !src) return;
    var md;
    try { md = JSON.parse(src.textContent || '""'); } catch (e) { md = src.textContent || ""; }
    var txt = btn.querySelector(".copy-md__txt");
    function ok() {
      btn.classList.add("is-copied");
      var prev = txt ? txt.textContent : null;
      if (txt) txt.textContent = "Copied";
      setTimeout(function () {
        btn.classList.remove("is-copied");
        if (txt && prev != null) txt.textContent = prev;
      }, 1400);
    }
    function fallback() {
      var ta = document.createElement("textarea");
      ta.value = md;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      try { document.execCommand("copy"); ok(); } catch (e) {}
      document.body.removeChild(ta);
    }
    btn.addEventListener("click", function () {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(md).then(ok, fallback);
      } else {
        fallback();
      }
    });
  })();

  // Mermaid pan/zoom + fullscreen. Mermaid renders <pre class="mermaid"> into an
  // <svg> asynchronously (client-side, CDN); we watch for that SVG and wrap each
  // diagram with wheel-zoom / drag-pan / fullscreen — no extra library.
  (function () {
    if (!document.querySelector("pre.mermaid")) return;

    function enhance(pre) {
      if (!pre.querySelector("svg")) return;
      // Controls must live OUTSIDE the <pre>: mermaid overwrites pre.innerHTML
      // (sometimes more than once), which wipes anything appended inside it. So
      // wrap the pre and hang the toolbar on the wrapper instead. The wrapper's
      // presence is also the idempotency guard.
      if (pre.parentElement && pre.parentElement.classList.contains("mermaid-wrap")) return;
      var wrap = document.createElement("div");
      wrap.className = "mermaid-wrap";
      pre.parentNode.insertBefore(wrap, pre);
      wrap.appendChild(pre);
      pre.classList.add("zoomable");
      pre.setAttribute("tabindex", "0");

      var state = { scale: 1, x: 0, y: 0 };
      function apply() {
        // Query the svg fresh — mermaid may replace it (e.g. on theme change).
        var s = pre.querySelector("svg");
        if (s) {
          s.style.transform =
            "translate(" + state.x + "px," + state.y + "px) scale(" + state.scale + ")";
        }
      }
      function clampScale(s) { return Math.min(8, Math.max(0.2, s)); }

      // Zoom toward a point (px,py) in the pre's local coordinates.
      function zoomAt(factor, px, py) {
        var next = clampScale(state.scale * factor);
        var ratio = next / state.scale;
        state.x = px - ratio * (px - state.x);
        state.y = py - ratio * (py - state.y);
        state.scale = next;
        apply();
      }
      // Center the diagram at scale 1 within the current pre box (works for
      // both the normal column and fullscreen, where the pre fills the
      // viewport). Centering lives in the transform so it stays consistent with
      // the zoom/pan math (transform-origin is the svg's top-left).
      function fit() {
        var s = pre.querySelector("svg");
        if (!s) return;
        state.scale = 1;
        state.x = 0;
        state.y = 0;
        s.style.transform = "";
        var sr = s.getBoundingClientRect();
        state.x = Math.max(0, (pre.clientWidth - sr.width) / 2);
        state.y = Math.max(0, (pre.clientHeight - sr.height) / 2);
        apply();
      }
      function reset() { fit(); }

      pre.addEventListener("wheel", function (e) {
        e.preventDefault();
        var rect = pre.getBoundingClientRect();
        zoomAt(e.deltaY < 0 ? 1.15 : 1 / 1.15, e.clientX - rect.left, e.clientY - rect.top);
      }, { passive: false });

      // Drag to pan.
      var dragging = false, sx = 0, sy = 0, ox = 0, oy = 0;
      pre.addEventListener("mousedown", function (e) {
        if (e.target.closest(".mermaid-controls")) return;
        dragging = true; sx = e.clientX; sy = e.clientY; ox = state.x; oy = state.y;
        pre.classList.add("grabbing");
        e.preventDefault();
      });
      window.addEventListener("mousemove", function (e) {
        if (!dragging) return;
        state.x = ox + (e.clientX - sx);
        state.y = oy + (e.clientY - sy);
        apply();
      });
      window.addEventListener("mouseup", function () {
        dragging = false; pre.classList.remove("grabbing");
      });

      // Touch: one-finger pan, two-finger pinch-zoom. Mobile has no hover, so
      // the controls stay visible there (CSS @media (hover: none)) and these
      // gestures make the diagram actually navigable once zoomed.
      var tPan = false, tsx = 0, tsy = 0, tox = 0, toy = 0, pinch = 0;
      function touchDist(t) {
        return Math.hypot(t[0].clientX - t[1].clientX, t[0].clientY - t[1].clientY);
      }
      pre.addEventListener("touchstart", function (e) {
        if (e.target.closest(".mermaid-controls")) return;
        if (e.touches.length === 1) {
          tPan = true; tsx = e.touches[0].clientX; tsy = e.touches[0].clientY;
          tox = state.x; toy = state.y;
        } else if (e.touches.length === 2) {
          tPan = false; pinch = touchDist(e.touches);
        }
      }, { passive: true });
      pre.addEventListener("touchmove", function (e) {
        if (e.touches.length === 2 && pinch > 0) {
          e.preventDefault();
          var rect = pre.getBoundingClientRect();
          var cx = (e.touches[0].clientX + e.touches[1].clientX) / 2 - rect.left;
          var cy = (e.touches[0].clientY + e.touches[1].clientY) / 2 - rect.top;
          var d = touchDist(e.touches);
          zoomAt(d / pinch, cx, cy);
          pinch = d;
        } else if (tPan && e.touches.length === 1) {
          e.preventDefault();
          state.x = tox + (e.touches[0].clientX - tsx);
          state.y = toy + (e.touches[0].clientY - tsy);
          apply();
        }
      }, { passive: false });
      pre.addEventListener("touchend", function (e) {
        if (e.touches.length === 0) { tPan = false; pinch = 0; }
        else if (e.touches.length === 1) {
          // A pinch dropped to one finger → resume panning from here.
          tPan = true; tsx = e.touches[0].clientX; tsy = e.touches[0].clientY;
          tox = state.x; toy = state.y; pinch = 0;
        }
      }, { passive: true });

      // Controls toolbar.
      var controls = document.createElement("div");
      controls.className = "mermaid-controls";
      function btn(label, title, onClick) {
        var b = document.createElement("button");
        b.type = "button";
        b.textContent = label;
        b.title = title;
        b.setAttribute("aria-label", title);
        b.addEventListener("click", function (e) { e.stopPropagation(); onClick(); });
        controls.appendChild(b);
      }
      btn("+", "Zoom in", function () {
        var r = pre.getBoundingClientRect(); zoomAt(1.2, r.width / 2, r.height / 2);
      });
      btn("−", "Zoom out", function () {
        var r = pre.getBoundingClientRect(); zoomAt(1 / 1.2, r.width / 2, r.height / 2);
      });
      btn("⟲", "Reset", reset);
      btn("⛶", "Fullscreen", function () {
        // Fullscreen the wrapper so the toolbar stays visible in fullscreen.
        if (document.fullscreenElement === wrap) {
          if (document.exitFullscreen) document.exitFullscreen();
        } else if (wrap.requestFullscreen) {
          wrap.requestFullscreen();
        }
      });
      wrap.appendChild(controls);

      // Center on first paint and whenever fullscreen toggles (the pre box
      // changes size). rAF so layout has settled before we measure.
      fit();
      requestAnimationFrame(fit);
      document.addEventListener("fullscreenchange", function () {
        requestAnimationFrame(fit);
      });
    }

    // Enhance every diagram that already has its <svg>. Idempotent (enhance
    // no-ops on an already-zoomable pre), so it is safe to call repeatedly.
    function enhanceAll() {
      document.querySelectorAll("pre.mermaid").forEach(function (p) {
        // Isolate failures: one diagram erroring must not block the others.
        try { enhance(p); } catch (e) { console.error("mermaid enhance:", e); }
      });
    }

    // mermaid renders asynchronously. We attach the toolbar through three
    // independent triggers so a missed one never leaves a diagram uncontrolled:
    //   1. the explicit "done" event the page fires after mermaid.run() resolves,
    //   2. a DOM observer catching the injected <svg>,
    //   3. timed sweeps as a final backstop.
    document.addEventListener("mdview:mermaid-done", enhanceAll);
    var obs = new MutationObserver(enhanceAll);
    obs.observe(document.body, { childList: true, subtree: true });
    [200, 800, 2000, 4000].forEach(function (t) { setTimeout(enhanceAll, t); });
    enhanceAll();
  })();

  // TOC scrollspy: highlight the "On this page" link matching the heading
  // currently in view while the reader scrolls a file page.
  (function () {
    var toc = document.querySelector(".toc");
    var article = document.querySelector(".fg-prose");
    if (!toc || !article) return;

    var links = Array.prototype.slice.call(toc.querySelectorAll("a[href^='#']"));
    if (!links.length) return;

    var linkByHash = {};
    links.forEach(function (a) { linkByHash[a.getAttribute("href")] = a; });

    var headings = links
      .map(function (a) { return document.getElementById(a.getAttribute("href").slice(1)); })
      .filter(Boolean);
    if (!headings.length) return;

    var current = null;
    function setActive(hash) {
      if (hash === current) return;
      if (current && linkByHash[current]) linkByHash[current].classList.remove("active");
      current = hash;
      if (current && linkByHash[current]) linkByHash[current].classList.add("active");
    }

    var observer = new IntersectionObserver(
      function (entries) {
        var visible = entries.filter(function (e) { return e.isIntersecting; });
        if (!visible.length) return;
        // Highest-on-page visible heading wins.
        visible.sort(function (a, b) { return a.boundingClientRect.top - b.boundingClientRect.top; });
        setActive("#" + visible[0].target.id);
      },
      { rootMargin: "0px 0px -70% 0px", threshold: 0 }
    );
    headings.forEach(function (h) { observer.observe(h); });
  })();

  // Live reload: reload-signal over WebSocket, full-page reload (PRD FR-19, Phase 1).
  function connect() {
    var proto = location.protocol === "https:" ? "wss:" : "ws:";
    var ws = new WebSocket(proto + "//" + location.host + "/ws");
    ws.onmessage = function (ev) {
      if (ev.data === "reload") location.reload();
    };
    ws.onclose = function () { setTimeout(connect, 3000); };
    ws.onerror = function () { try { ws.close(); } catch (e) {} };
  }
  connect();
})();
