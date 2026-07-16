/* ============================================================
   ATELIER SWITCHER — single-theme handoff build.
   Theme is locked to "atelier". Exposes the axes that remain
   meaningful within one theme: Scheme (light/dark) · Accent ·
   Density · Numbers · Typeface. All persist to localStorage
   and stay in sync across the demo pages.
   ============================================================ */
(function () {
  var html = document.documentElement;

  var PAGES = [
    { href: "core.html",      label: "Core" },
    { href: "editorial.html", label: "Editorial" },
    { href: "task.html",      label: "Task" },
    { href: "crm.html",       label: "CRM" },
    { href: "financial.html", label: "Financial" },
    { href: "media.html",     label: "Media" },
    { href: "elements.html",  label: "Elements" }
  ];

  var ACCENTS   = [["", "Ember"], ["clay", "Clay"], ["honey", "Honey"]];
  var TYPEFACES = [["", "Manrope"], ["grotesk", "Grotesk"], ["system", "System"]];

  var get = function (k, d) { return localStorage.getItem("fg-" + k) || d; };
  var set = function (k, v) { if (v) localStorage.setItem("fg-" + k, v); else localStorage.removeItem("fg-" + k); };

  function apply() {
    html.setAttribute("data-theme", "atelier");          // locked
    html.setAttribute("data-scheme", get("scheme", "light"));
    var a = get("accent", "");   a ? html.setAttribute("data-accent", a) : html.removeAttribute("data-accent");
    var d = get("density", "");  d ? html.setAttribute("data-density", d) : html.removeAttribute("data-density");
    var n = get("num", "");      n ? html.setAttribute("data-num-font", n) : html.removeAttribute("data-num-font");
    var t = get("typeface", ""); t ? html.setAttribute("data-typeface-set", t) : html.removeAttribute("data-typeface-set");
  }
  apply(); // run ASAP (in <head>) to prevent a flash

  function segment(label, options, current, onPick) {
    var grp = document.createElement("div"); grp.className = "demo-grp";
    var lbl = document.createElement("span"); lbl.className = "demo-grp__lbl"; lbl.textContent = label; grp.appendChild(lbl);
    var seg = document.createElement("div"); seg.className = "seg";
    options.forEach(function (opt) {
      var b = document.createElement("button");
      b.dataset.v = opt[0]; b.textContent = opt[1];
      b.setAttribute("aria-pressed", opt[0] === current);
      b.onclick = function () {
        [].forEach.call(seg.children, function (c) { c.setAttribute("aria-pressed", c.dataset.v === opt[0]); });
        onPick(opt[0]);
      };
      seg.appendChild(b);
    });
    grp.appendChild(seg); return grp;
  }

  var PALETTE_ICON = '<svg width="17" height="17" viewBox="0 0 17 17" fill="none" xmlns="http://www.w3.org/2000/svg">' +
    '<path d="M8.5 1.5c-3.87 0-7 3.13-7 7s3.13 7 7 7c.83 0 1.5-.67 1.5-1.5 0-.4-.16-.76-.41-1.03-.25-.27-.41-.63-.41-1.03 0-.83.67-1.5 1.5-1.5h1.77c1.72 0 3.11-1.4 3.11-3.11C15.5 4.03 12.42 1.5 8.5 1.5z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>' +
    '<circle cx="5" cy="7.2" r="1" fill="currentColor"/><circle cx="8.3" cy="4.8" r="1" fill="currentColor"/>' +
    '<circle cx="11.8" cy="6.6" r="1" fill="currentColor"/><circle cx="5.3" cy="10.8" r="1" fill="currentColor"/></svg>';

  function build() {
    var host = document.getElementById("fg-bar");
    if (!host) return;
    host.className = "demo-bar";
    host.innerHTML = "";

    var here = location.pathname.split("/").pop() || "core.html";
    var nav = document.createElement("nav"); nav.className = "demo-nav";
    PAGES.forEach(function (p) {
      var a = document.createElement("a");
      a.href = p.href; a.textContent = p.label;
      if (p.href === here) a.setAttribute("aria-current", "page");
      nav.appendChild(a);
    });

    var spacer = document.createElement("div"); spacer.className = "demo-bar__spacer";
    var wrap = document.createElement("div"); wrap.className = "demo-palette";
    var trigger = document.createElement("button");
    trigger.className = "demo-palette__btn"; trigger.type = "button";
    trigger.title = "Customize"; trigger.setAttribute("aria-label", "Customize");
    trigger.innerHTML = PALETTE_ICON;

    var pop = document.createElement("div"); pop.className = "demo-palette__pop"; pop.hidden = true;
    var ctrls = document.createElement("div"); ctrls.className = "demo-ctrls";

    ctrls.appendChild(segment("Scheme", [["light", "Light"], ["dark", "Dark"]], get("scheme", "light"),
      function (v) { set("scheme", v); apply(); }));
    ctrls.appendChild(segment("Accent", ACCENTS, get("accent", ""),
      function (v) { set("accent", v); apply(); }));
    ctrls.appendChild(segment("Typeface", TYPEFACES, get("typeface", ""),
      function (v) { set("typeface", v); apply(); }));
    ctrls.appendChild(segment("Density", [["", "Default"], ["compact", "Compact"]], get("density", ""),
      function (v) { set("density", v); apply(); }));
    ctrls.appendChild(segment("Numbers", [["mono", "Mono"], ["sans", "Sans"]], get("num", "mono"),
      function (v) { set("num", v === "mono" ? "" : v); apply(); }));

    pop.appendChild(ctrls);
    wrap.appendChild(trigger); wrap.appendChild(pop);

    trigger.onclick = function (e) { e.stopPropagation(); pop.hidden = !pop.hidden; trigger.setAttribute("aria-pressed", !pop.hidden); };
    document.addEventListener("click", function (e) { if (!pop.hidden && !wrap.contains(e.target)) { pop.hidden = true; trigger.setAttribute("aria-pressed", "false"); } });
    document.addEventListener("keydown", function (e) { if (e.key === "Escape" && !pop.hidden) { pop.hidden = true; trigger.setAttribute("aria-pressed", "false"); trigger.focus(); } });

    host.appendChild(spacer); host.appendChild(nav); host.appendChild(wrap);
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", build);
  else build();
})();
