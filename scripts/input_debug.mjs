// Отладка ввода в headless: фокус, латиница, кириллица, insertText, Ctrl+Enter.
import { createRequire } from 'module';
import { mkdirSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = '/home/z/my-project/repro_out_debug';
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

  // Создание заметки двойным кликом
  await page.mouse.dblclick(640, 380); await sleep(1500);
  const focus1 = await page.evaluate(() => {
    const el = document.activeElement;
    return el ? `${el.tagName}.${el.className}`.slice(0, 60) : 'none';
  });
  console.log('activeElement после dblclick:', focus1);

  // 1) Латиница через keyboard.type
  await page.keyboard.type('abc', { delay: 60 }); await sleep(600);
  await shot('d1_latin_typed');
  // 2) Кириллица через keyboard.insertText (IME-путь)
  await page.keyboard.insertText('тест'); await sleep(800);
  await shot('d2_inserttext');
  // 3) Enter (коммит заголовка → тело)
  await page.keyboard.press('Enter'); await sleep(900);
  await shot('d3_enter_commit_title');
  // 4) В теле: латиница + insertText кириллица
  await page.keyboard.type('xyz', { delay: 60 }); await sleep(500);
  await page.keyboard.insertText('привет'); await sleep(800);
  await shot('d4_body_typed');
  // 5) Ctrl+Enter
  await page.keyboard.press('Control+Enter'); await sleep(900);
  const focus2 = await page.evaluate(() => {
    const el = document.activeElement;
    return el ? `${el.tagName}.${el.className}`.slice(0, 60) : 'none';
  });
  console.log('activeElement после Ctrl+Enter:', focus2);
  await shot('d5_ctrl_enter');
  // 6) Ещё текст — идёт ли в редактор?
  await page.keyboard.insertText('два'); await sleep(800);
  await shot('d6_after_type_more');

  console.log('=== ЛОГИ ===');
  for (const l of logs.slice(-25)) console.log(l);
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО');
