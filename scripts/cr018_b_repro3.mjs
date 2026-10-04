// CR-018 v1.1: репро — часть 3 (палитра через Ctrl+P → поиск → вставка).
import { createRequire } from 'module';
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = '/home/z/my-project/repro_out';
const URL = 'https://danku13.github.io/CanvasDesk/app/';
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
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(6000);
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await click(729, 467, 1500);
  // Палитра: как в run 1 — два клика по чипу, затем Ctrl+P (фокус в поиск)
  await click(83, 415, 600);
  await click(83, 442, 900);
  await click(150, 75, 600);
  await shot('19a_search_clicked');
  for (const ch of ['a','u','t','h']) {
    await page.keyboard.press(ch);
    await page.waitForTimeout(150);
  }
  await page.waitForTimeout(1400);
  await shot('20_filtered');
  // Первый (единственный) результат — строка списка под поиском
  await click(180, 170, 1800);
  await shot('21_after_insert_click');
  await page.keyboard.press('Escape'); await page.waitForTimeout(800);
  await shot('22_node_normal');
} finally {
  await browser.close().catch(() => {});
  for (const l of logs) if (l.includes('pageerror') || l.includes('паника')) console.log(l);
}
