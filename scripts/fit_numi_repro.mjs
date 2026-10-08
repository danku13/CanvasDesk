// CR-027: фит на Numi-заметке с юнитами (сценарий проверки CR-019, «rub»).
import { createRequire } from 'module';
import { mkdirSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const URL = 'https://danku13.github.io/CanvasDesk/app/';
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
mkdirSync('/home/z/my-project/repro_out_numi', { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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

const browser = await chromium.launch({
  headless: true,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const shot = (n) => page.screenshot({ path: `/home/z/my-project/repro_out_numi/${n}.png` });
  await page.addInitScript(() => {
    try { localStorage.setItem('canvasdesk.config', 'onboarding_done = true'); } catch (e) {}
  });
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await sleep(8000);
  // Последовательность закрытия онбординга из успешного прогона fit_height
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await sleep(120);
    await page.mouse.down(); await sleep(60);
    await page.mouse.up(); await sleep(ms);
  };
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await click(729, 467, 1500);
  // Дожать тур по шагам (если жив): Escape ×3 + «Пустой холст»
  for (let i = 0; i < 3; i++) { await page.keyboard.press('Escape'); await sleep(400); }
  await click(790, 494, 900); // «Пустой холст» (центр кнопки шага 3)
  await shot('n0_canvas_clean');

  // Заметка-Numi-лист с юнитами (сценарий CR-019: умножение + «rub»)
  await page.mouse.dblclick(640, 380); await sleep(1600);
  await page.keyboard.type('sum', { delay: 40 });
  await page.keyboard.press('Enter'); await sleep(900);
  await shot('n0_body');

  const lines = [
    'a = 10 rub',
    'b = 2',
    'a * b',
    'c = a * b rub',
    'd = c + a rub',
    'd * 2',
    'e = 3 rub',
    'e * d rub',
  ];
  for (let i = 0; i < lines.length; i++) {
    await page.keyboard.type(lines[i], { delay: 15 });
    if (i < lines.length - 1) { await page.keyboard.press('Control+Enter'); await sleep(450); }
    await sleep(250);
    const m = await cardBottom(page, 760, 430);
    console.log(`[numi ${i + 1}] кромка=${m?.bottom} textBelow=${m?.textBelow} ref=${m?.ref}`);
    if (i === 0 || i === 3 || i === 6 || i === 7) await shot(`n${String(i + 1).padStart(2, '0')}`);
  }
  await page.mouse.click(200, 720); await sleep(1100);
  const m = await cardBottom(page, 760, 430);
  console.log(`[numi после коммита] кромка=${m?.bottom} textBelow=${m?.textBelow}`);
  await shot('n09_committed');
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО');
