// CR-027 (репродуктор координатора): Ctrl+Enter не расширяет высоту заметки,
// строки уходят под клип (сообщение владельца, post-wave handtest).
//
// Шаги: онбординг → пустой холст → двойной клик (создание заметки, FR-072:
// правка заголовка) → Enter (коммит заголовка → цепочка к телу) → текст →
// Control+Enter → текст → пиксель-скан силуэта карточки + скриншоты.
//
// Диагностика:
//  - карточка растёт после Ctrl+Enter → фит работает, вопрос закрыт;
//  - редактор закрылся после Ctrl+Enter (коммит) → winit-web модификаторы
//    (UR-001-02 / FR-100 W4);
//  - строки ниже нижней кромки карточки → клип без роста.
//
// Запуск: node scripts/ctrl_enter_repro.mjs

import { createRequire } from 'module';
import { mkdirSync, writeFileSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = process.env.OUT ?? '/home/z/my-project/repro_out_ctrlenter';
const URL = process.env.REPRO_URL ?? 'https://danku13.github.io/CanvasDesk/app/';
mkdirSync(OUT, { recursive: true });
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
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  const shot = (n) => page.screenshot({ path: `${OUT}/${n}.png` });
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await sleep(120);
    await page.mouse.down(); await sleep(60);
    await page.mouse.up(); await sleep(ms);
  };
  const dblclick = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await sleep(120);
    await page.mouse.dblclick(x, y); await sleep(ms);
  };
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await sleep(8000);
  await shot('00_onboarding');
  // Онбординг: продолжить без роли → пропустить тур → пустой холст
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await click(729, 467, 1500);
  await shot('01_empty_canvas');

  // Создание заметки двойным кликом по пустому холсту (центр)
  const CX = 640, CY = 380;
  await dblclick(CX, CY, 1500);
  await shot('02_title_editing');
  // Печать заголовка + Enter (коммит → цепочка title_then_body → правка тела)
  await page.keyboard.type('Заголовок', { delay: 40 });
  await sleep(300);
  await page.keyboard.press('Enter'); await sleep(800);
  await shot('03_body_editing_started');

  // Тело: первая строка
  await page.keyboard.type('строка один', { delay: 30 });
  await sleep(400);
  await shot('04_line1_typed');

  // ГЛАВНЫЙ ШАГ: Control+Enter (хорд)
  await page.keyboard.press('Control+Enter'); await sleep(900);
  await shot('05_after_ctrl_enter');

  // Печать второй строки — идёт ли текст в редактор?
  await page.keyboard.type('строка два', { delay: 30 });
  await sleep(500);
  await shot('06_line2_typed');

  // Контрольный Shift+Enter — задокументированный перенос строки
  await page.keyboard.press('Shift+Enter'); await sleep(700);
  await page.keyboard.type('строка три', { delay: 30 });
  await sleep(500);
  await shot('07_after_shift_enter');

  // Клик мимо — коммит; финальный силуэт
  await click(200, 650, 1200);
  await shot('08_committed');

  // Пиксель-скан: нижняя кромка карточки и текст под ней (по PNG 08)
  // Делаем через canvas.toDataURL прямо из страницы (точные пиксели кадра)
  const dataUrl = await page.evaluate(() => {
    const c = document.querySelector('body > canvas');
    return c ? c.toDataURL('image/png') : null;
  });
  if (dataUrl) writeFileSync(`${OUT}/frame_final.png`, Buffer.from(dataUrl.split(',')[1], 'base64'));

  console.log('=== ЛОГИ КОНСОЛИ (усечённые) ===');
  for (const l of logs.slice(-40)) console.log(l);
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО: скриншоты в ' + OUT);
