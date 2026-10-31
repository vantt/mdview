// Render mỗi <section class="frame" id="..."> trong một file HTML thành PNG 1920x1080.
//   node render.mjs <frames.html> <out-dir>
// Playwright lấy từ ~/projects/skillstation/web/node_modules.
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const require = createRequire(path.join(os.homedir(), 'projects/skillstation/web/package.json'));
const { chromium } = require('playwright');
const [html, outDir] = process.argv.slice(2);

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
await page.goto(pathToFileURL(path.resolve(html)).href, { waitUntil: 'networkidle' });
await page.evaluate(() => document.fonts.ready);
for (const id of await page.$$eval('section.frame', els => els.map(e => e.id))) {
  await page.locator(`#${id}`).screenshot({ path: path.join(outDir, `${id}.png`) });
  console.log(`${id}.png`);
}
await browser.close();
