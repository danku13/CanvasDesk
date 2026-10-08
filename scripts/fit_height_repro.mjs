// CR-027: фит-тест высоты — 8 строк через Ctrl+Enter, замер нижней кромки
// карточки после каждого шага (пиксель-скан внутри страницы).
import { createRequire } from 'module';
import { mkdirSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = '/home/z/my-project/repro_out_fit';
mkdirSync(OUT, { recursive: true });
const URL = 'https://danku13.github.io/CanvasDesk/app/';
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';

const browser = await chromium.launch({
  headless: true,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
const logs = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Нижняя кромка карточки: скан колонки x по canvas-пикселям.
// Опорный цвет — заливка карточки в точке (rx, ry).
async function cardBottom(page, rx, ry) {
  return page.evaluate(({ rx, ry }) => {
    const c = document.querySelector('body > canvas');
    if (!c) return null;
    const off = document.createElement('canvas');
    off.width = c.width; off.height = c.height;
    const ctx = off.getContext('2d');
    ctx.drawImage(c, 0, 0);
    const scale = c.width / c.clientWidth;
    const px = (x, y) => {
      const d = ctx.getImageData(Math.round(x * scale), Math.round(y * scale), 1, 1).data;
      return [d[0], d[1], d[2]];
    };
    const ref = px(rx, ry);
    const near = (a, b) => Math.abs(a[0] - b[0]) <= 12 && Math.abs(a[1] - b[1]) <= 12 && Math.abs(a[2] - b[2]) <= 12;
    let bottom = -1;
    for (let y = ry; y < c.clientHeight; y++) {
      if (near(px(rx, y), ref)) bottom = y;
    }
    // Текст ниже кромки: светлые пиксели (глифы) в полосе x±120 на y кромка+3..+40
    let textBelow = 0;
    for (let y = bottom + 3; y < Math.min(bottom + 40, c.clientHeight); y++) {
      for (let x = rx - 120; x < rx + 120; x++) {
        const p = px(x, y);
        if (p[0] > 150 && p[1] > 150 && p[2] > 150) textBelow++;
      }
    }
    return { ref, bottom, textBelow };
  }, { rx, ry });
}

try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  const shot = (n) => page.screenshot({ path: `${OUT}/${n}.png` });
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await sleep(8000);
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await sleep(120);
    await page.mouse.down(); await sleep(60);
    await page.mouse.up(); await sleep(ms);
  };
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await click(729, 467, 1500);

  // Создание заметки (двойной клик) → заголовок → Enter → тело
  await page.mouse.dblclick(640, 380); await sleep(1500);
  await page.keyboard.type('fit', { delay: 40 });
  await page.keyboard.press('Enter'); await sleep(900);

  // 8 строк; замер кромки после каждой. x=760 — внутри карточки (тело),
  // ry=430 — заведомо внутри карточки (опорный цвет заливки).
  for (let i = 1; i <= 8; i++) {
    await page.keyboard.type(`line${i} test`, { delay: 20 });
    if (i < 8) { await page.keyboard.press('Control+Enter'); await sleep(500); }
    await sleep(300);
    const m = await cardBottom(page, 760, 430);
    console.log(`[строка ${i}] кромка=${m?.bottom} textBelow=${m?.textBelow} ref=${m?.ref}`);
    await shot(`f${String(i).padStart(2, '0')}`);
  }
  // Коммит (клик мимо) и финальный замер
  await click(250, 650, 1200);
  const m = await cardBottom(page, 760, 430);
  console.log(`[после коммита] кромка=${m?.bottom} textBelow=${m?.textBelow}`);
  await shot('f09_committed');
  for (const l of logs.slice(-15)) console.log(l);
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО');
