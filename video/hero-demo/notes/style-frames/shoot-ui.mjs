// Chụp giao diện mdview thật (theme tối) để đặt vào style frame.
//   MDVIEW_TOKEN=... node shoot-ui.mjs <url> <out.png>
// Gặp trang /login thì đăng nhập bằng MDVIEW_TOKEN (mặc định `demo`).
// EXTRA_CSS=<file.css>: chèn thêm CSS (vd. bản sửa app.css chưa phát hành).
// Playwright lấy từ ~/projects/skillstation/web/node_modules.
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';

const require = createRequire(path.join(os.homedir(), 'projects/skillstation/web/package.json'));
const { chromium } = require('playwright');
const [url, out] = process.argv.slice(2);

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: Number(process.env.VW || 1600), height: Number(process.env.VH || 1000) }, deviceScaleFactor: 2, colorScheme: 'dark' });
await page.goto(url, { waitUntil: 'networkidle' });
if (new URL(page.url()).pathname.startsWith('/login')) {
  await page.fill('input', process.env.MDVIEW_TOKEN || 'demo');
  await Promise.all([page.waitForNavigation({ waitUntil: 'networkidle' }), page.keyboard.press('Enter')]);
  await page.goto(url, { waitUntil: 'networkidle' });
}
if (process.env.EXTRA_CSS) await page.addStyleTag({ content: fs.readFileSync(process.env.EXTRA_CSS, 'utf8') });
await page.waitForSelector('svg[id^="mermaid"], .mermaid svg', { timeout: 15000 }).catch(() => {});
await page.waitForTimeout(800);
await page.screenshot({ path: out });
await browser.close();
