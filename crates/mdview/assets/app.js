// Theme toggle (cycles light → dark) with persistence, and WebSocket live reload.
(function () {
  "use strict";

  function applyTheme(t) {
    var dark = t === "dark" || (t === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
  }

  var toggle = document.getElementById("theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var cur = document.documentElement.getAttribute("data-theme");
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
