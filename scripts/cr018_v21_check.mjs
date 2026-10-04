// CR-018 v2.1 приёмка: S3-нода — норма, допечатывание расчётных строк
// (inline-правка кликом по последней строке), большая таблица, правка.
import { createRequire } from 'module';
import { spawn } from 'child_process';
import { mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const DIST = process.env.WASM_UI_DIST ?? path.join(REPO, 'target/dist');
const OUT = process.env.WASM_UI_OUT ?? path.join(REPO, 'target/wasm_cr018_v21');
const PORT = process.env.WASM_UI_PORT ?? '8087';
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html`;
mkdirSync(OUT, { recursive: true });
const server = spawn('python3', ['-m', 'http.server', PORT, '--bind', '127.0.0.1'], { cwd: DIST, stdio: 'ignore' });
process.on('exit', () => { try { server.kill(); } catch {} });
setTimeout(() => process.exit(3), 300000).unref();

const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
const browser = await chromium.launch({
  headless: false,
  ...(CHROME ? { executablePath: CHROME } : {}),
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
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(7000);
  await page.mouse.click(633, 497); await page.waitForTimeout(900);
  await page.mouse.click(812, 299); await page.waitForTimeout(700);
  // S3-шаблон
  await page.mouse.click(100, 373); await page.waitForTimeout(900);
  await page.mouse.click(360, 655); await page.waitForTimeout(1500);
  await page.mouse.click(30, 770); await page.waitForTimeout(500);
  await shot('20_s3_normal');
  // Клик по ПОСЛЕДНЕЙ строке данных («object_size =») — inline-правка,
  // каретка в конце строки (клик правее текста прижимается к концу).
  await page.mouse.click(960, 568); await page.waitForTimeout(1200);
  const lines = [
    '',
    'load = requests_per_sec / latency',
    'storage_cost = object_size * 4',
    'total_cost = load * storage_cost',
  ];
  for (let i = 0; i < lines.length; i++) {
    await page.keyboard.type(lines[i], { delay: 3 });
    if (i < lines.length - 1) {
      await page.keyboard.down('Shift'); await page.keyboard.press('Enter'); await page.keyboard.up('Shift');
    }
  }
  await page.waitForTimeout(400);
  await shot('22_append_edit');
  await page.keyboard.press('Enter'); await page.waitForTimeout(1500);
  await page.mouse.click(30, 770); await page.waitForTimeout(500);
  await shot('23_big_normal');
  // Снова правка: документируем нестабильность вёрстки (desc пропал,
  // строки взлетели наверх) на БОЛЬШОЙ ноде
  await page.mouse.click(960, 568); await page.waitForTimeout(1200);
  await shot('24_big_edit');
  await page.keyboard.press('Escape'); await page.waitForTimeout(600);
  console.log('[шаг] готово');
} finally {
  await browser.close().catch(() => {});
  server.kill();
  for (const l of logs) if (l.includes('pageerror')) console.log(l);
}
