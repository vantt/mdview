/* ============================================================
   CANONICAL DESIGN CONTRACT — elements.js  (Tier-4 companion)

   OPTIONAL. Additive only: locks the DOM structure of the
   highest-reuse / highest-drift-risk roles into real custom
   elements, so pages compose them by TAG instead of hand-typing
   markup each time. Every element renders the SAME `.fg-*`
   classes/parts documented in SPEC.md and used by cards/demo —
   this is not a second implementation, it is the contract's own
   markup, emitted by code instead of by hand.

   - Renders into LIGHT DOM (no shadow root) — the page's linked
     styles.css (contract → components → patterns → theme) applies
     normally, so theme-switching still needs zero markup change.
   - Raw `.fg-*` markup keeps working untouched. Nothing here is
     required — delete this file and every page still renders,
     just back to hand-authored markup.
   - Load AFTER styles.css, once per page:
       <script src="elements.js"></script>

   Elements in this file: fg-button · fg-chip · fg-badge ·
   fg-status · fg-switch · fg-kpi · fg-card · fg-banner ·
   fg-caveat · fg-field · fg-avatar · fg-avatar-cluster ·
   fg-tabs / fg-tab.

   Not covered (compose from core roles + raw markup instead —
   content varies too much per page to lock into one shape):
   table, modal, cmdk, kanban, task-table, and all domain patterns.
   ============================================================ */
(function () {
  'use strict';

  // Detach and return this element's children matching `slotName`
  // ('' / undefined = the default slot: everything WITHOUT a
  // slot="" attribute, including bare text). Call the named-slot
  // extraction BEFORE the default one so it isn't swept up by it.
  function take(host, slotName) {
    const out = [];
    [...host.childNodes].forEach((node) => {
      const named = node.nodeType === 1 && node.hasAttribute && node.hasAttribute('slot')
        ? node.getAttribute('slot')
        : null;
      if (slotName) {
        if (named === slotName) out.push(node);
      } else if (!named) {
        out.push(node);
      }
    });
    out.forEach((n) => n.remove());
    return out;
  }

  function makeEl(tag, className) {
    const e = document.createElement(tag);
    if (className) e.className = className;
    return e;
  }

  function once(proto) {
    const orig = proto.connectedCallback;
    proto.connectedCallback = function () {
      if (this._fgBuilt) return;
      this._fgBuilt = true;
      // Parser-created instances (the primary use case: hand-authored static
      // HTML) fire connectedCallback as part of the parser's per-element
      // custom-element-reactions processing \u2014 BEFORE this element's later
      // children (e.g. an <input> appearing after other content) have been
      // parsed and appended. A microtask is NOT enough: microtask checkpoints
      // run repeatedly *during* synchronous parsing, not only once at the end,
      // so `queueMicrotask` can still fire mid-parse. A macrotask (setTimeout)
      // is scheduled after the parser's current task \u2014 in practice after the
      // whole synchronous parse for a static page \u2014 so children are present
      // by the time we read them. JS-created instances (already fully built
      // before insertion) just pay a harmless extra tick.
      setTimeout(() => orig.call(this), 0);
    };
  }

  // This platform's editor instrumentation tracks the ORIGINAL authored
  // text of an element and expects it to stay a direct child of whatever
  // it stamped — re-parenting that text into a newly created nested
  // element (e.g. wrapping a label in a fresh <button>) races with it and
  // leaves a duplicate floating copy behind. So for button/chip-toggle/
  // switch/tab below, the HOST element itself becomes the styled,
  // interactive control (classes + role + keyboard handling added
  // directly onto it) instead of nesting a second real <button> and
  // moving the label into it. Authored text never moves.
  function injectOnce(id, css) {
    if (document.getElementById(id)) return;
    const style = document.createElement('style');
    style.id = id;
    style.textContent = css;
    document.head.appendChild(style);
  }
  injectOnce('fg-elements-style', [
    'fg-switch { font-size: var(--type-body-sm-size); color: var(--color-text); }',
    'fg-switch:focus-visible, fg-chip[toggle]:focus-visible { outline: var(--focus-width) solid var(--focus-color); outline-offset: var(--focus-offset); }',
  ].join('\n'));

  // ---------------------------------------------------------- button
  class FgButton extends HTMLElement {
    connectedCallback() {
      const variant = this.getAttribute('variant') || 'primary';
      this.classList.add('fg-btn', 'fg-btn--' + variant);
      if (!this.hasAttribute('role')) this.setAttribute('role', 'button');
      this.addEventListener('keydown', (e) => {
        if (this.hasAttribute('disabled')) return;
        if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') { e.preventDefault(); this.click(); }
      });
      this.addEventListener('click', (e) => {
        if (this.hasAttribute('disabled')) { e.stopImmediatePropagation(); e.preventDefault(); }
      }, true);
      this._syncDisabled();
      this._ready = true;
    }
    static get observedAttributes() { return ['disabled', 'variant']; }
    attributeChangedCallback(name, _ov, nv) {
      if (!this._ready) return;
      if (name === 'variant') {
        [...this.classList].filter((c) => c.indexOf('fg-btn--') === 0).forEach((c) => this.classList.remove(c));
        this.classList.add('fg-btn--' + (nv || 'primary'));
      }
      if (name === 'disabled') this._syncDisabled();
    }
    _syncDisabled() {
      const dis = this.hasAttribute('disabled');
      this.setAttribute('aria-disabled', dis ? 'true' : 'false');
      this.tabIndex = dis ? -1 : 0;
      this.style.opacity = dis ? '.5' : '';
      this.style.cursor = dis ? 'not-allowed' : '';
      this.style.pointerEvents = dis ? 'none' : '';
    }
  }
  once(FgButton.prototype);
  customElements.define('fg-button', FgButton);

  // ---------------------------------------------------------- chip
  class FgChip extends HTMLElement {
    connectedCallback() {
      const tone = this.getAttribute('tone') || 'neutral';
      const toggle = this.hasAttribute('toggle');
      if (toggle) {
        this.classList.add('fg-chip', 'fg-chip--toggle');
        if (this.hasAttribute('on')) this.classList.add('fg-chip--on');
        this.setAttribute('role', 'button');
        this.tabIndex = 0;
        const fire = () => {
          const on = this.classList.toggle('fg-chip--on');
          if (on) this.setAttribute('on', ''); else this.removeAttribute('on');
          this.dispatchEvent(new CustomEvent('fg-change', { detail: { on }, bubbles: true }));
        };
        this.addEventListener('click', fire);
        this.addEventListener('keydown', (e) => {
          if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') { e.preventDefault(); fire(); }
        });
      } else {
        this.classList.add('fg-chip', 'fg-chip--' + tone);
      }
    }
  }
  once(FgChip.prototype);
  customElements.define('fg-chip', FgChip);

  // ---------------------------------------------------------- badge
  class FgBadge extends HTMLElement {
    connectedCallback() {
      const accent = this.getAttribute('tone') === 'accent';
      const content = take(this, '');
      this.classList.add('fg-badge');
      if (accent) this.classList.add('fg-badge--accent');
      content.forEach((n) => this.append(n));
    }
  }
  once(FgBadge.prototype);
  customElements.define('fg-badge', FgBadge);

  // ---------------------------------------------------------- status
  class FgStatus extends HTMLElement {
    connectedCallback() {
      const state = this.getAttribute('state') || 'ready'; // ready | warn | blocked
      const label = take(this, '');
      this.classList.add('fg-status', 'fg-status--' + state);
      this.append(makeEl('span', 'fg-status__dot'));
      label.forEach((n) => this.append(n));
    }
  }
  once(FgStatus.prototype);
  customElements.define('fg-status', FgStatus);

  // ---------------------------------------------------------- switch
  class FgSwitch extends HTMLElement {
    connectedCallback() {
      const labelAttr = this.getAttribute('label');
      this.classList.add('fg-switch');
      if (this.hasAttribute('on')) this.classList.add('fg-switch--on');
      this.setAttribute('role', 'switch');
      this.setAttribute('aria-checked', this.hasAttribute('on') ? 'true' : 'false');
      this.tabIndex = 0;
      const track = makeEl('span', 'fg-switch__track');
      track.append(makeEl('span', 'fg-switch__dot'));
      this.prepend(track); // authored label text (if any) stays put; track just moves to the front
      if (labelAttr != null) {
        const labelSpan = makeEl('span', 'fg-switch__label');
        labelSpan.textContent = labelAttr;
        this.append(labelSpan);
      }
      const fire = () => {
        const on = this.classList.toggle('fg-switch--on');
        this.setAttribute('aria-checked', on ? 'true' : 'false');
        if (on) this.setAttribute('on', ''); else this.removeAttribute('on');
        this.dispatchEvent(new CustomEvent('fg-change', { detail: { on }, bubbles: true }));
      };
      this.addEventListener('click', fire);
      this.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') { e.preventDefault(); fire(); }
      });
    }
  }
  once(FgSwitch.prototype);
  customElements.define('fg-switch', FgSwitch);

  // ---------------------------------------------------------- kpi
  class FgKpi extends HTMLElement {
    connectedCallback() {
      const label = this.getAttribute('label') || '';
      const value = this.getAttribute('value') || '';
      const deltaText = this.getAttribute('delta');
      const deltaDir = this.getAttribute('delta-dir') || 'flat'; // up | down | flat
      this.classList.add('fg-kpi');
      const l = makeEl('span', 'fg-kpi__label t-label'); l.textContent = label;
      const v = makeEl('span', 'fg-kpi__value'); v.textContent = value;
      this.append(l, v);
      if (deltaText) {
        const foot = makeEl('span', 'fg-kpi__foot');
        const d = makeEl('span', 'fg-delta fg-delta--' + deltaDir); d.textContent = deltaText;
        foot.append(d);
        this.append(foot);
      }
    }
  }
  once(FgKpi.prototype);
  customElements.define('fg-kpi', FgKpi);

  // ---------------------------------------------------------- card
  class FgCard extends HTMLElement {
    connectedCallback() {
      const title = this.getAttribute('title');
      const sub = this.getAttribute('sub');
      const rule = this.hasAttribute('rule');
      const sunken = this.hasAttribute('sunken');
      const foot = take(this, 'foot');
      const body = take(this, '');
      this.classList.add('fg-card');
      if (rule) this.classList.add('fg-card--rule');
      if (sunken) this.classList.add('fg-card--sunken');
      if (title) {
        const head = makeEl('div', 'fg-card__head');
        const t = makeEl('span', 'fg-card__title'); t.textContent = title;
        head.append(t);
        this.append(head);
      }
      if (sub) {
        const s = makeEl('p', 'fg-card__sub t-body-sm'); s.textContent = sub;
        this.append(s);
      }
      // Author-provided body/foot content must stay a DIRECT child of the
      // host (this platform's editor instrumentation re-homes it there) —
      // append directly instead of wrapping in fresh .fg-card__body/__foot
      // divs. Trade-off: body/foot content follows the card's own
      // --space-4 rhythm rather than the wrapper's tighter/row layout.
      body.forEach((n) => this.append(n));
      foot.forEach((n) => this.append(n));
    }
  }
  once(FgCard.prototype);
  customElements.define('fg-card', FgCard);

  // ---------------------------------------------------------- banner
  class FgBanner extends HTMLElement {
    connectedCallback() {
      const tone = this.getAttribute('tone') || 'info'; // info | warning | danger | success
      const actionLabel = this.getAttribute('action-label');
      const actionHref = this.getAttribute('action-href') || '#';
      const body = take(this, '');
      this.classList.add('fg-banner', 'fg-banner--' + tone);
      const dot = makeEl('span', 'fg-banner__dot');
      dot.style.order = '-1'; // pin first: immune to the platform re-homing tracked siblings
      this.append(dot);
      // Author-provided message must stay a direct child (see fg-card note
      // above) — append directly instead of wrapping in a fresh
      // .fg-banner__body span. It keeps the flex default `order: 0`.
      body.forEach((n) => this.append(n));
      if (actionLabel) {
        const a = makeEl('a', 'fg-banner__act');
        a.href = actionHref;
        a.textContent = actionLabel;
        a.style.order = '1'; // pin last — DOM append order alone isn't honored (see note above)
        this.append(a);
      }
    }
  }
  once(FgBanner.prototype);
  customElements.define('fg-banner', FgBanner);

  // ---------------------------------------------------------- caveat
  class FgCaveat extends HTMLElement {
    connectedCallback() {
      const tone = this.getAttribute('tone') || 'info'; // info | warn | rule
      const body = take(this, '');
      this.classList.add('fg-caveat', 'fg-caveat--' + tone);
      body.forEach((n) => this.append(n));
    }
  }
  once(FgCaveat.prototype);
  customElements.define('fg-caveat', FgCaveat);

  // ---------------------------------------------------------- field
  class FgField extends HTMLElement {
    connectedCallback() {
      const labelText = this.getAttribute('label');
      const required = this.hasAttribute('required');
      const hint = this.getAttribute('hint');
      const error = this.getAttribute('error');
      const control = take(this, ''); // author-provided <input>/<select>/<textarea>
      this.classList.add('fg-field');
      if (labelText) {
        const l = makeEl('label', 'fg-field__label t-label');
        l.style.order = '-1'; // pin first — see fg-banner note on `order` vs DOM position
        l.append(document.createTextNode(labelText + (required ? ' ' : '')));
        if (required) {
          const r = makeEl('span', 'fg-field__req');
          r.textContent = '*';
          l.append(r);
        }
        this.append(l);
      }
      control.forEach((c) => {
        if (c.nodeType !== 1) { this.append(c); return; }
        if (c.tagName === 'SELECT') {
          // Skip the .fg-select wrapper + custom chevron (would wrap
          // author-provided content in a fresh element — same bug as
          // fg-card/fg-banner above). Falls back to the select's native
          // appearance; still fully functional.
          this.append(c);
        } else {
          c.classList.add('fg-input');
          if (c.tagName === 'TEXTAREA') c.classList.add('fg-input--area');
          this.append(c);
        }
      });
      if (hint && !error) {
        const h = makeEl('span', 'fg-field__hint');
        h.style.order = '1'; // pin last — the tracked <input>/<select> ends up appended before this
        h.textContent = hint;
        this.append(h);
      }
      if (error) {
        const e = makeEl('span', 'fg-field__error');
        e.style.order = '1';
        e.textContent = error;
        this.append(e);
      }
    }
  }
  once(FgField.prototype);
  customElements.define('fg-field', FgField);

  // ---------------------------------------------------------- avatar
  class FgAvatar extends HTMLElement {
    connectedCallback() {
      const more = this.hasAttribute('more');
      const content = take(this, '');
      this.classList.add('fg-avatar');
      if (more) this.classList.add('fg-avatar--more');
      content.forEach((n) => this.append(n));
    }
  }
  once(FgAvatar.prototype);
  customElements.define('fg-avatar', FgAvatar);

  class FgAvatarCluster extends HTMLElement {
    connectedCallback() { this.classList.add('fg-avatar-cluster'); }
  }
  once(FgAvatarCluster.prototype);
  customElements.define('fg-avatar-cluster', FgAvatarCluster);

  // ---------------------------------------------------------- tabs / tab
  class FgTab extends HTMLElement {
    connectedCallback() {
      this.classList.add('fg-tab');
      if (this.hasAttribute('active')) this.classList.add('fg-tab--on');
      this.setAttribute('role', 'tab');
      this.tabIndex = 0;
      this.addEventListener('click', () => this._activate());
      this.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') { e.preventDefault(); this._activate(); }
      });
    }
    _activate() {
      const tabs = this.closest('fg-tabs');
      if (tabs) {
        [...tabs.children].forEach((t) => {
          if (t.tagName === 'FG-TAB' && t !== this) { t.classList.remove('fg-tab--on'); t.removeAttribute('active'); }
        });
      }
      this.classList.add('fg-tab--on');
      this.setAttribute('active', '');
      if (tabs) {
        tabs.dispatchEvent(new CustomEvent('fg-tab-change', {
          detail: { label: this.textContent.trim() }, bubbles: true,
        }));
      }
    }
  }
  once(FgTab.prototype);
  customElements.define('fg-tab', FgTab);

  class FgTabs extends HTMLElement {
    connectedCallback() { this.classList.add('fg-tabs'); }
  }
  once(FgTabs.prototype);
  customElements.define('fg-tabs', FgTabs);
})();
