// CR-018 v1.1: репро — часть 4 (OPFS-сев ?canvas=auth.canvas → нода на канвасе).
import { createRequire } from 'module';
import { readFileSync } from 'fs';
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = process.env.OUT ?? '/home/z/my-project/repro_out';
const URL = process.env.REPRO_URL ?? 'https://danku13.github.io/CanvasDesk/app/?canvas=authrepro';
const CANVAS_JSON = readFileSync('/home/z/my-project/auth.canvas', 'utf8');
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';

const browser = await chromium.launch({
  headless: false,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  const shot = (n) => page.screenshot({ path: `${OUT}/${n}.png` });
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await page.waitForTimeout(120);
    await page.mouse.down(); await page.waitForTimeout(60);
    await page.mouse.up(); await page.waitForTimeout(ms);
  };
  // Сев OPFS до старта приложения (document-start): файл ждёт ?canvas=authrepro
  await page.addInitScript(async (content) => {
    try {
      const root = await navigator.storage.getDirectory();
      const fh = await root.getFileHandle('authrepro.canvas', { create: true });
      const w = await fh.createWritable();
      await w.write(content);
      await w.close();
    } catch (e) { console.error('[seed] OPFS fail', e); }
  }, CANVAS_JSON);
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(7000);
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await page.waitForTimeout(1500);
  await shot('30_node_from_opfs');
  // Правка: клик по строке token_verify (вторая строка params)
  await click(560, 242, 1500);
  await shot('31_edit_mode_B');
  await page.keyboard.type('9', { delay: 40 });
  await page.waitForTimeout(700);
  await shot('32_edit_typed_B');
  await page.keyboard.press('Escape');
  await page.waitForTimeout(1200);
  await shot('33_after_edit');
} finally {
  await browser.close().catch(() => {});
  for (const l of logs) if (l.includes('pageerror') || l.includes('[seed]') || l.includes('паника')) console.log(l);
}
