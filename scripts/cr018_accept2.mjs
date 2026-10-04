// CR-018 приёмка v3: инстанцирование «Хранилище (S3)» + hover строки.
import { createRequire } from 'module';
import { spawn } from 'child_process';
import { existsSync, mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const DIST = process.env.WASM_UI_DIST ?? path.join(REPO, 'target/dist');
const OUT = process.env.WASM_UI_OUT ?? path.join(REPO, 'target/wasm_ui_cr018');
const PORT = process.env.WASM_UI_PORT ?? '8087';
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html`;
const HOVER = (process.env.HOVER ?? '0,0').split(',').map(Number);
mkdirSync(OUT, { recursive: true });
const server = spawn('python3', ['-m', 'http.server', PORT, '--bind', '127.0.0.1'], { cwd: DIST, stdio: 'ignore' });
process.on('exit', () => { try { server.kill(); } catch {} });
setTimeout(() => process.exit(3), 300000).unref();

const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
const browser = await chromium.launch({
  headless: false, // headless captureScreenshot не композитит WebGPU-канвас
  ...(existsSync(CHROME) ? { executablePath: CHROME } : {}),
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
  // Категория «Бакенд · 14» → шаблон «Хранилище (S3)»
  await page.mouse.click(100, 373); await page.waitForTimeout(900);
  await shot('10_s3_list');
  await page.mouse.click(360, 655); await page.waitForTimeout(1500);
  await shot('11_s3_node');
  if (HOVER[0] > 0) {
    await page.mouse.move(HOVER[0], HOVER[1]);
    await page.waitForTimeout(700);
    await shot('12_s3_hover');
    await page.mouse.move(30, 770);
    await page.waitForTimeout(500);
    await shot('13_s3_nohover');
  }
  console.log('[шаг] готово');
} finally {
  await browser.close().catch(() => {});
  server.kill();
  for (const l of logs) console.log(l);
}
