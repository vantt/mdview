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

      // "Up one level" affordance when not at root.
      if (focus) {
        var up = el("button", "chap-up", "↑ ..");
        up.addEventListener("click", function () { focus = dirOf(focus); render(); });
        root.appendChild(up);
      }

      // Subfolders first (zoom in), sorted.
      Object.keys(folders).sort().forEach(function (name) {
        var b = el("button", "chap-folder", name + "/");
        b.addEventListener("click", function () {
          focus = focus ? focus + "/" + name : name;
          render();
        });
        root.appendChild(b);
      });

      // Files by title (fallback to basename), sorted by label; current active.
      here
        .map(function (f) { return { f: f, label: f.t && f.t.length ? f.t : baseOf(f.p) }; })
        .sort(function (a, b) { return a.label.localeCompare(b.label); })
        .forEach(function (item) {
          var a = el("a", "chap-file" + (item.f.p === current ? " active" : ""), item.label);
          a.href = "/p/" + pid + "/" + item.f.p;
          root.appendChild(a);
        });
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

  // Mermaid pan/zoom + fullscreen. Mermaid renders <pre class="mermaid"> into an
  // <svg> asynchronously (client-side, CDN); we watch for that SVG and wrap each
  // diagram with wheel-zoom / drag-pan / fullscreen — no extra library.
  (function () {
    var pres = document.querySelectorAll("pre.mermaid");
    if (!pres.length) return;

    function enhance(pre) {
      var svg = pre.querySelector("svg");
      if (!svg || pre.classList.contains("zoomable")) return;
      pre.classList.add("zoomable");
      pre.setAttribute("tabindex", "0");

      var state = { scale: 1, x: 0, y: 0 };
      function apply() {
        svg.style.transform =
          "translate(" + state.x + "px," + state.y + "px) scale(" + state.scale + ")";
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
      function reset() { state = { scale: 1, x: 0, y: 0 }; apply(); }

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
        if (document.fullscreenElement === pre) {
          if (document.exitFullscreen) document.exitFullscreen();
        } else if (pre.requestFullscreen) {
          pre.requestFullscreen();
        }
      });
      pre.appendChild(controls);
    }

    pres.forEach(function (pre) {
      if (pre.querySelector("svg")) { enhance(pre); return; }
      // mermaid injects the <svg> later; observe until it appears.
      var obs = new MutationObserver(function () {
        if (pre.querySelector("svg")) { obs.disconnect(); enhance(pre); }
      });
      obs.observe(pre, { childList: true, subtree: true });
    });
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
