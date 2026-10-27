// Record the browser scenes of the hero video against the demo daemon (sandbox.sh up).
//
//   node video/hero-demo/record.mjs [scene ...]     # default: all three
//
//   scene4-read   page opens; slow scroll past the table to the diagram; zoom the diagram
//   scene5-edit   Edit → change the `max_retries` value → save → the page re-renders
//   scene6-link   click the plan's link to an ADR → the linked doc opens
//
// Output: <out>/<scene>.webm (1536x864), plus <scene>.mp4 (1920x1080) when ffmpeg is on PATH. Chromium maps
// build-01 to 127.0.0.1 itself, so /etc/hosts is not needed here. scene5 edits the
// plan; it puts the file back as it was when it ends. The plan is whatever is in the demo project:
// the fixture after `sandbox.sh up`, or the agent's own plan right after scene 3 (render.sh does
// that, so the cut from the link to the page matches). The project's CLAUDE.md template makes the
// agent's plan have the parts these scenes use: a Mermaid diagram, a YAML `max_retries`, ADR links.
//
// Environment:
//   DEMO_HOST, DEMO_PORT, DEMO_TOKEN, DEMO_DIR   as in sandbox.sh
//   DEMO_OUT       output folder (default video/hero-demo/out)
//   DEMO_THEME     dark (default) or light
//   DEMO_HEADED=1  show the browser while recording
//
// Playwright: `npm i -D playwright` anywhere and set NODE_PATH, or reuse a checkout
// that has it (PLAYWRIGHT_FROM=<dir with package.json>).
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const host = process.env.DEMO_HOST || 'build-01';
const port = process.env.DEMO_PORT || '7710';
const token = process.env.DEMO_TOKEN || 'hero-demo';
const demoDir = process.env.DEMO_DIR || path.join(process.env.TMPDIR || '/tmp', 'mdview-hero-demo');
const outDir = path.resolve(process.env.DEMO_OUT || path.join(here, 'out'));
const theme = process.env.DEMO_THEME === 'light' ? 'light' : 'dark';
const base = `http://${host}:${port}`;
const page1 = `${base}/p/acme-payments/docs/plans/payment-retry.md`;
const planFile = path.join(demoDir, 'acme-payments/docs/plans/payment-retry.md');
const planBackup = fs.readFileSync(planFile);
let newValue = '3';

const from = process.env.PLAYWRIGHT_FROM || path.join(os.homedir(), 'projects/skillstation/web');
const { chromium } = createRequire(path.join(from, 'package.json'))('playwright');

// Pages render at 1536x864 CSS pixels; ffmpeg scales the recording to 1920x1080,
// which reads like a 1920 screen at 125% zoom. (Playwright records at the
// viewport's CSS size, so a bigger video size would only add grey borders.)
const viewport = { width: 1536, height: 864 };
const pause = ms => new Promise(r => setTimeout(r, ms));

async function smoothScrollTo(page, y, ms = 1600) {
  await page.evaluate(([to, dur]) => new Promise(done => {
    const start = window.scrollY, t0 = performance.now();
    const ease = t => (t < .5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
    (function step(now) {
      const t = Math.min(1, (now - t0) / dur);
      window.scrollTo(0, start + (to - start) * ease(t));
      t < 1 ? requestAnimationFrame(step) : done();
    })(t0);
  }), [y, ms]);
}

async function glideTo(page, locator, steps = 40) {
  const b = await locator.boundingBox();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps });
}

// Playwright draws no mouse pointer, so the edit draws one: log every click (time in the video,
// position in the 1920x1080 output) to <scene>.clicks.json.
let clicks = [];
let sceneStart = 0;
async function clickLogged(page, locator, label) {
  const b = await locator.boundingBox();
  const scale = 1920 / viewport.width;
  clicks.push({ t: (Date.now() - sceneStart) / 1000, x: Math.round((b.x + b.width / 2) * scale), y: Math.round((b.y + b.height / 2) * scale), label });
  await locator.click();
}

const scenes = {
  async 'scene4-read'(page) {
    await page.goto(page1, { waitUntil: 'networkidle' });
    await page.waitForSelector('.mermaid-done, .mermaid svg', { timeout: 15000 }).catch(() => {});
    await pause(1500);
    const diagram = page.locator('.mermaid-wrap').first();
    const top = await diagram.evaluate(el => el.getBoundingClientRect().top + window.scrollY - 140);
    await smoothScrollTo(page, top, 2200);
    await pause(700);
    await glideTo(page, diagram);
    const plus = page.locator('.mermaid-controls button').first();
    await glideTo(page, plus, 25);
    for (let i = 0; i < 2; i++) { await clickLogged(page, plus, 'zoom'); await pause(450); }
    await pause(1500);
  },

  async 'scene5-edit'(page) {
    await page.goto(page1, { waitUntil: 'networkidle' });
    const code = page.locator('pre', { hasText: 'max_retries' }).first();
    const y = await code.evaluate(el => el.getBoundingClientRect().top + window.scrollY - 260);
    await smoothScrollTo(page, y, 1200);
    await pause(1200);
    const edit = page.locator('#edit-md');
    await glideTo(page, edit, 30);
    await clickLogged(page, edit, 'edit');
    const editor = page.locator('.md-editor .cm-content, .md-editor__ta').first();
    await editor.waitFor();
    await pause(900);
    const line = page.locator('.md-editor .cm-line', { hasText: /max_retries:\s*\d+/ }).first();
    if (await line.count()) {
      await line.scrollIntoViewIfNeeded();
      await glideTo(page, line, 25);
      const old = (await line.innerText()).match(/max_retries:\s*(\d+)/)[1];
      newValue = old === '3' ? '5' : '3';
      await line.click();
      await page.keyboard.press('End');
      for (let i = 0; i < old.length; i++) await page.keyboard.press('Backspace', { delay: 120 });
      await page.keyboard.type(newValue, { delay: 160 });
    } else {
      // Textarea fallback: replace in place.
      newValue = '3';
      await editor.evaluate(ta => { ta.value = ta.value.replace(/max_retries:\s*\d+/, 'max_retries: 3'); ta.dispatchEvent(new Event('input', { bubbles: true })); });
    }
    await pause(900);
    await page.keyboard.press('Control+s');
    await page.waitForLoadState('networkidle');
    await pause(1200);
    const fixed = page.locator('pre', { hasText: `max_retries: ${newValue}` }).first();
    if (await fixed.count()) {
      const y2 = await fixed.evaluate(el => el.getBoundingClientRect().top + window.scrollY - 260);
      await smoothScrollTo(page, y2, 900);
    }
    await pause(1800);
  },

  async 'scene6-link'(page) {
    await page.goto(page1, { waitUntil: 'networkidle' });
    await pause(1000);
    let link = page.locator('article a[href*="adr"]').first();
    if (!(await link.count())) link = page.locator('article a').first();
    await glideTo(page, link, 35);
    await pause(400);
    await clickLogged(page, link, 'link');
    await page.waitForLoadState('networkidle');
    await pause(1800);
  },
};

const wanted = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(scenes);
fs.mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({
  headless: !process.env.DEMO_HEADED,
  args: [`--host-resolver-rules=MAP ${host} 127.0.0.1`],
});

// Sign in once, outside any recording.
const login = await browser.newContext({ viewport });
const lp = await login.newPage();
await lp.goto(`${base}/login`);
await lp.fill('input', token);
await Promise.all([lp.waitForLoadState('networkidle'), lp.keyboard.press('Enter')]);
const storageState = await login.storageState();
await login.close();

try {
  for (const name of wanted) {
    if (!scenes[name]) throw new Error(`unknown scene ${name}`);
    const ctx = await browser.newContext({
      viewport, colorScheme: theme, storageState,
      recordVideo: { dir: outDir, size: viewport },
    });
    clicks = [];
    sceneStart = Date.now();
    const page = await ctx.newPage();
    try {
      await scenes[name](page);
    } finally {
      const video = page.video();
      await ctx.close();
      const webm = path.join(outDir, `${name}.webm`);
      fs.renameSync(await video.path(), webm);
      fs.writeFileSync(path.join(outDir, `${name}.clicks.json`), JSON.stringify({ clicks }, null, 2) + '\n');
      try {
        execFileSync('ffmpeg', ['-v', 'error', '-y', '-i', webm, '-vf', 'scale=1920:1080:flags=lanczos', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18', webm.replace(/\.webm$/, '.mp4')]);
      } catch { /* ffmpeg missing: keep the webm */ }
      if (name === 'scene5-edit') fs.writeFileSync(planFile, planBackup);
      console.log(webm);
    }
  }
} finally {
  await browser.close();
}
