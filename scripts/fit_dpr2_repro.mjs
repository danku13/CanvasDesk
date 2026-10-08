// CR-027: фит при DPR 1.5 — корректные координаты (Skip онбординга),
// зонд от точки создания заметки. Сравнение с DPR 1.
import { createRequire } from 'module';
import { mkdirSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const URL = 'https://danku13.github.io/CanvasDesk/app/';
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
mkdirSync('/home/z/my-project/repro_out_dpr2', { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Зонд: опорный цвет в (rx,ry), ищем последнюю строку этого цвета вниз,
// затем считаем светлые пиксели (текст) под кромкой в колонке ±100.
async function cardBottom(page, rx, ry) {
  return page.evaluate(({ rx, ry }) => {
    const c = document.querySelector('body > canvas');
    if (!c) return null;
    const off = document.createElement('canvas');
    off.width = c.width; off.height = c.height;
    const ctx = off.getContext('2d');
    ctx.drawImage(c, 0, 0);
    const k = c.width / c.clientWidth;
    const px = (x, y) => {
      const d = ctx.getImageData(Math.round(x * k), Math.round(y * k), 1, 1).data;
      return [d[0], d[1], d[2]];
    };
    const ref = px(rx, ry);
    const near = (a, b) => Math.abs(a[0]-b[0]) <= 10 && Math.abs(a[1]-b[1]) <= 10 && Math.abs(a[2]-b[2]) <= 10;
    let bottom = -1;
    for (let y = ry; y < c.clientHeight; y++) if (near(px(rx, y), ref)) bottom = y;
    let textBelow = 0;
    if (bottom > 0) {
      for (let y = bottom + 4; y < Math.min(bottom + 44, c.clientHeight); y++)
        for (let x = rx - 100; x < rx + 100; x++) {
          const p = px(x, y);
          if (p[0] > 150 && p[1] > 150 && p[2] > 150) textBelow++;
        }
    }
    return { ref, bottom, textBelow };
  }, { rx, ry });
}

async function runCase(browser, label, dsf, outDir) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dsf });
  const shot = (n) => page.screenshot({ path: `${outDir}/${label}_${n}.png` });
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await sleep(9000);
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await sleep(150);
    await page.mouse.down(); await sleep(70);
    await page.mouse.up(); await sleep(ms);
  };
  // Онбординг при DPR≠1 (координаты из фактических кадров):
  // 1) «Пропустить» тура → экран роли; 2) «Продолжить без роли» → (иногда
  // снова тур-карточка) «Пропустить» → пустой холст
  await shot('s0_welcome');
  await click(Math.round(1280 * 0.72), Math.round(800 * 0.314), 1300);
  await shot('s1_role');
  await click(Math.round(1280 * 0.508), Math.round(800 * 0.645), 2000);
  // Тур перезапускается после роли — гасим по Escape (Tour.onKey → skip)
  await page.keyboard.press('Escape'); await sleep(1000);
  await page.keyboard.press('Escape'); await sleep(1000);
  await page.keyboard.press('Escape'); await sleep(800);
  // Если остался шаг «Начните с шаблона» — «Пустой холст» (доля 0.62, 0.63)
  await click(Math.round(1280 * 0.624), Math.round(800 * 0.629), 1000);
  await shot('s2b_canvas');

  // Создание заметки двойным кликом; запоминаем точку
  const NX = 640, NY = 380;
  await page.mouse.dblclick(NX, NY); await sleep(1600);
  await shot('s3_note_title');
  await page.keyboard.type('fit', { delay: 40 });
  await page.keyboard.press('Enter'); await sleep(900);
  await shot('s4_body');

  // Зонд от точки заметки: реф на (NX+25, NY+25) — внутри шапки/тела
  const rx = NX + 25, ry = NY + 30;
  for (let i = 1; i <= 8; i++) {
    await page.keyboard.type(`line${i} test`, { delay: 20 });
    if (i < 8) { await page.keyboard.press('Control+Enter'); await sleep(500); }
    await sleep(300);
    const m = await cardBottom(page, rx, ry);
    console.log(`[${label} строка ${i}] кромка=${m?.bottom} textBelow=${m?.textBelow} ref=${m?.ref}`);
    await shot(`f${String(i).padStart(2, '0')}`);
  }
  await click(150, 720, 1200);
  const m = await cardBottom(page, rx, ry);
  console.log(`[${label} после коммита] кромка=${m?.bottom} textBelow=${m?.textBelow}`);
  await shot('f09_committed');
  await page.close();
}

const browser = await chromium.launch({
  headless: true,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
try {
  await runCase(browser, 'dpr150b', 1.5, '/home/z/my-project/repro_out_dpr2');
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО');
