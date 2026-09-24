// Аудитор вёрстки (WASM-самопроверка по AGENTS.md): обходит поверхности и
// состояния нод, снимает скриншоты в target/wasm_ui/audit/. Каждый шаг —
// свежая загрузка страницы (детерминированное состояние), действия —
// клавиатура/мышь, оракул — визуальный аудит скриншотов + пиксельные диффы.
//
// Запуск: SCENARIO=scripts/wasm_audit.mjs scripts/wasm_ui_test.sh --no-build
// Группы шагов: AUDIT_GROUPS=probe,ru_panels,ru_nodes,en_panels,en_nodes
// (по умолчанию probe — калибровочный).
//
// Контракты те же, что у wasm_ui_scenario.mjs: сервер-в-сценарии, kill в
// finally, watchdog; Chromium headed под Xvfb (поднимает wasm_ui_test.sh).
import { createRequire } from 'module';
import { spawn } from 'child_process';
import { existsSync, mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');

const DIST = process.env.WASM_UI_DIST ?? path.join(REPO, 'target/dist');
const OUT = path.join(REPO, 'target/wasm_ui/audit');
const PORT = process.env.WASM_UI_PORT ?? '8081';
const BASE = `http://127.0.0.1:${PORT}`;
const WATCHDOG_MS = Number(process.env.WASM_UI_WATCHDOG_S ?? 900) * 1000;

// Калиброванные координаты (env COORDS_JSON перекрывает: {"SKIP":[x,y],...})
const C = Object.assign(
  {
    SKIP: [812, 299], // «Пропустить» онбординга
    HELP: [1163, 66], // кнопка «?» — после WEB_TOOLBAR_INSET y=48..84
    THEME: [1204, 66], // угловая кнопка темы
    GEAR: [1249, 66], // угловая кнопка настроек ⚙
    // пункты меню «?» — меню якорится к верху кнопки «?» (y=48):
    // первый пункт больше не под DOM-тулбаром (фикс D2)
    MI_DOCS: [1000, 68],
    MI_ONBOARD: [1000, 96],
    MI_GALLERY: [1000, 124],
    MI_KIT: [1035, 152],
    MI_ADMIN: [1035, 180],
    CANVAS: [640, 400], // центр канваса
  },
  process.env.COORDS_JSON ? JSON.parse(process.env.COORDS_JSON) : {},
);

const STEPS = [
  // ---------- калибровка ----------
  { g: 'probe', name: '00_onboarding', url: '/', actions: [['wait', 1200], ['shot']] },
  { g: 'probe', name: '01_idle', url: '/', actions: [['click', C.SKIP], ['wait', 900], ['shot']] },
  { g: 'probe', name: '02_help_menu', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 900], ['shot']] },
  { g: 'probe2', name: '03_corner_mid', url: '/', actions: [['click', C.SKIP], ['click', [1204, 44]], ['wait', 1000], ['shot']] },
  { g: 'probe2', name: '04_corner_right', url: '/', actions: [['click', C.SKIP], ['click', [1249, 44]], ['wait', 1000], ['shot']] },
  { g: 'probe2', name: '05_docs_lower', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', [1000, 42]], ['wait', 1200], ['shot']] },

  // ---------- RU: панели ----------
  { g: 'ru_panels', name: '10_empty', url: '/', actions: [['click', C.SKIP], ['wait', 900], ['shot']] },
  { g: 'ru_panels', name: '11_hotkeys', url: '/', actions: [['click', C.SKIP], ['key', 'F1'], ['wait', 900], ['shot']] },
  { g: 'ru_panels', name: '12_search', url: '/', actions: [['click', C.SKIP], ['key', 'Control+F'], ['wait', 900], ['shot']] },
  { g: 'ru_panels', name: '12c_synthetic_cyr', url: '/', actions: [['click', C.SKIP], ['key', 'Control+F'], ['wait', 600], ['synth', 'выручка'], ['wait', 1200], ['shot']] },
  { g: 'ru_panels', name: '12b_search_typed', url: '/', actions: [['click', C.SKIP], ['key', 'Control+F'], ['wait', 600], ['type', 'выручка'], ['wait', 1200], ['shot']] },
  { g: 'ru_panels', name: '13_palette', url: '/', actions: [['click', C.SKIP], ['key', 'Control+P'], ['wait', 1000], ['shot']] },
  { g: 'ru_panels', name: '14_wheel', url: '/', actions: [['click', C.SKIP], ['shiftclick', C.CANVAS], ['wait', 1000], ['shot']] },
  { g: 'ru_panels', name: '15_settings', url: '/', actions: [['click', C.SKIP], ['click', C.GEAR], ['wait', 1000], ['shot']] },
  { g: 'ru_panels', name: '15b_settings_look', url: '/', actions: [['click', C.SKIP], ['click', C.GEAR], ['wait', 800], ['click', [500, 326]], ['wait', 900], ['shot']] },
  { g: 'ru_panels', name: '16_docs', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_DOCS], ['wait', 1200], ['shot']] },
  { g: 'ru_panels', name: '17_gallery', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_GALLERY], ['wait', 1500], ['shot']] },
  { g: 'ru_panels', name: '18_kit', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_KIT], ['wait', 1200], ['shot']] },
  { g: 'ru_panels', name: '19_admin', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_ADMIN], ['wait', 1200], ['shot']] },
  { g: 'ru_panels', name: '20_flowmap', url: '/', actions: [['click', C.SKIP], ['key', 'Control+Shift+M'], ['wait', 1000], ['shot']] },
  { g: 'ru_panels', name: '21_whatif', url: '/', actions: [['click', C.SKIP], ['key', 'Control+Shift+I'], ['wait', 1000], ['shot']] },
  { g: 'ru_panels', name: '22_hud', url: '/', actions: [['click', C.SKIP], ['key', 'F3'], ['wait', 800], ['shot']] },

  // ---------- RU: ноды (шаблонная схема + ручная заметка) ----------
  { g: 'ru_nodes', name: '30_seed_ue', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 2000], ['shot']] },
  { g: 'ru_nodes', name: '31_seed_toast', url: '/?template=com.canvasdesk.pa-funnel-conv', actions: [['click', C.SKIP], ['wait', 900], ['shot'], ['wait', 2500], ['shot2', '31b_seed_toast_late']] },
  { g: 'ru_nodes', name: '32_lod_out', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 1500], ['wheel', [0, -1600]], ['wait', 1200], ['shot']] },
  { g: 'ru_nodes', name: '33_lod_in', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 1500], ['wheel', [0, 900]], ['wait', 1200], ['shot']] },
  { g: 'ru_nodes', name: '34_editor', url: '/', actions: [['click', C.SKIP], ['dclick', C.CANVAS], ['wait', 900], ['type', 'Выручка = сделки × средний чек'], ['wait', 600], ['shot']] },
  { g: 'ru_nodes', name: '35_note', url: '/', actions: [['click', C.SKIP], ['dclick', C.CANVAS], ['wait', 700], ['type', 'Проверка вёрстки заметки'], ['key', 'Enter'], ['wait', 900], ['shot']] },
  { g: 'ru_nodes', name: '36_ctx_menu', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 1500], ['rclick', C.CANVAS], ['wait', 1000], ['shot']] },
  { g: 'ru_nodes', name: '37_stage', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 1500], ['click', C.CANVAS], ['wait', 1200], ['shot']] },
  { g: 'ru_nodes', name: '38_bottleneck', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 1500], ['key', 'Control+B'], ['wait', 1200], ['shot']] },

  // ---------- EN: панели ----------
  { g: 'en_panels', name: '50_lang_switch', url: '/', actions: [['click', C.SKIP], ['click', C.GEAR], ['wait', 800], ['click', [500, 326]], ['wait', 700], ['shot']] },
  { g: 'en_panels', name: '51_hotkeys', url: '/', actions: [['click', C.SKIP], ['key', 'F1'], ['wait', 900], ['shot']] },
  { g: 'en_panels', name: '52_search', url: '/', actions: [['click', C.SKIP], ['key', 'Control+F'], ['wait', 600], ['type', 'revenue'], ['wait', 1200], ['shot']] },
  { g: 'en_panels', name: '53_palette', url: '/', actions: [['click', C.SKIP], ['key', 'Control+P'], ['wait', 1000], ['shot']] },
  { g: 'en_panels', name: '54_settings', url: '/', actions: [['click', C.SKIP], ['click', C.GEAR], ['wait', 1000], ['shot']] },
  { g: 'en_panels', name: '55_help_menu', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 900], ['shot']] },
  { g: 'en_panels', name: '56_docs', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_DOCS], ['wait', 1200], ['shot']] },
  { g: 'en_panels', name: '57_gallery', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_GALLERY], ['wait', 1500], ['shot']] },
  { g: 'en_panels', name: '58_kit', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_KIT], ['wait', 1200], ['shot']] },
  { g: 'en_panels', name: '59_admin', url: '/', actions: [['click', C.SKIP], ['click', C.HELP], ['wait', 600], ['click', C.MI_ADMIN], ['wait', 1200], ['shot']] },
  { g: 'en_panels', name: '60_whatif', url: '/', actions: [['click', C.SKIP], ['key', 'Control+Shift+I'], ['wait', 1000], ['shot']] },

  // ---------- EN: ноды ----------
  { g: 'en_nodes', name: '70_seed_ue', url: '/?template=com.canvasdesk.scheme.unit-economics', actions: [['click', C.SKIP], ['wait', 2000], ['shot']] },
  { g: 'en_nodes', name: '71_editor', url: '/', actions: [['click', C.SKIP], ['dclick', C.CANVAS], ['wait', 900], ['type', 'Revenue = deals × avg check'], ['wait', 600], ['shot']] },
];

const GROUPS = (process.env.AUDIT_GROUPS ?? 'probe').split(',').map((s) => s.trim());
const ONLY = (process.env.AUDIT_ONLY ?? '').split(',').map((s) => s.trim()).filter(Boolean);
const steps = STEPS.filter(
  (s) => GROUPS.includes(s.g) && (ONLY.length === 0 || ONLY.includes(s.name)),
);
if (steps.length === 0) {
  console.error(`нет шагов для групп: ${GROUPS.join(', ')}`);
  process.exit(2);
}

mkdirSync(OUT, { recursive: true });
const server = spawn('python3', ['-m', 'http.server', PORT, '--bind', '127.0.0.1'], {
  cwd: DIST, stdio: 'ignore',
});
const stop = () => { try { server.kill(); } catch { /* уже мёртв */ } };
process.on('exit', stop);
const watchdog = setTimeout(() => {
  console.error(`[аудит] watchdog ${WATCHDOG_MS / 1000} с — принудительный выход`);
  process.exit(3);
}, WATCHDOG_MS);
watchdog.unref();

const CHROME = process.env.WASM_UI_CHROME
  ?? '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
const browser = await chromium.launch({
  headless: false, // живой композитор под Xvfb: headless не композитит WebGPU
  ...(existsSync(CHROME) ? { executablePath: CHROME } : {}),
  args: [
    '--no-sandbox', '--disable-gpu-sandbox',
    '--enable-unsafe-webgpu', '--enable-features=Vulkan',
    '--use-vulkan=swiftshader', '--use-webgpu-adapter=swiftshader',
    '--window-size=1280,800', '--window-position=0,0',
  ],
});

const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  // Диагностика DOM-ввода (мост кириллицы): какие события реально диспатчатся
  if (process.env.AUDIT_EVENTS) {
    await page.addInitScript(() => {
      for (const type of ['keydown', 'keypress', 'beforeinput', 'input', 'textInput']) {
        document.addEventListener(type, (e) => {
          const data = e.data ?? e.key ?? '';
          console.log(`[EV] ${type} «${data}»`);
        }, true);
      }
    });
  }
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror@${step_name}] ${e.message}`));

  const done = [];
  let step_name = '';
  for (const step of steps) {
    step_name = step.name;
    const file = `${OUT}/${step.name}.png`;
    try {
      await page.goto(BASE + step.url, { waitUntil: 'load' });
      await page.waitForSelector('body > canvas', { timeout: 90000 });
      await page.waitForTimeout(7000); // dev-wasm: первый кадр
      for (const a of step.actions) {
        const [kind, ...args] = a;
        if (kind === 'click') {
          await page.mouse.move(args[0][0], args[0][1]);
          await page.waitForTimeout(150);
          await page.mouse.click(args[0][0], args[0][1]);
          await page.waitForTimeout(args[1] ?? 500);
        } else if (kind === 'shiftclick') {
          await page.keyboard.down('Shift');
          await page.mouse.click(args[0][0], args[0][1]);
          await page.keyboard.up('Shift');
          await page.waitForTimeout(args[1] ?? 500);
        } else if (kind === 'dclick') {
          await page.mouse.dblclick(args[0][0], args[0][1]);
          await page.waitForTimeout(args[1] ?? 500);
        } else if (kind === 'rclick') {
          await page.mouse.click(args[0][0], args[0][1], { button: 'right' });
          await page.waitForTimeout(args[1] ?? 500);
        } else if (kind === 'synth') {
          // Синтетический keydown с key=символ — путь РЕАЛЬНОЙ раскладки:
          // браузер даёт keydown с key=«в» → winit Key::Character
          for (const ch of args[0]) {
            await page.evaluate((ch) => {
              const canvas = document.querySelector('body > canvas');
              canvas.dispatchEvent(new KeyboardEvent('keydown', {
                key: ch, code: 'KeyD', bubbles: true, cancelable: true,
              }));
              canvas.dispatchEvent(new KeyboardEvent('keyup', {
                key: ch, code: 'KeyD', bubbles: true, cancelable: true,
              }));
            }, ch);
            await page.waitForTimeout(120);
          }
        } else if (kind === 'key') {
          await page.keyboard.press(args[0]);
          await page.waitForTimeout(args[1] ?? 400);
        } else if (kind === 'type') {
          await page.keyboard.type(args[0], { delay: 35 });
          await page.waitForTimeout(args[1] ?? 300);
        } else if (kind === 'wheel') {
          await page.mouse.wheel(args[0][0], args[0][1]);
          await page.waitForTimeout(args[1] ?? 500);
        } else if (kind === 'move') {
          await page.mouse.move(args[0][0], args[0][1]);
          await page.waitForTimeout(args[1] ?? 200);
        } else if (kind === 'wait') {
          await page.waitForTimeout(args[0]);
        } else if (kind === 'shot') {
          await page.screenshot({ path: file });
        } else if (kind === 'shot2') {
          await page.screenshot({ path: `${OUT}/${args[0]}.png` });
        }
      }
      if (!step.actions.some((a) => a[0] === 'shot' || a[0] === 'shot2')) {
        await page.screenshot({ path: file });
      }
      done.push(`${step.name} ✓`);
      console.log(`[аудит] ${step.name} ✓`);
    } catch (e) {
      done.push(`${step.name} ✗ ${e.message.split('\n')[0]}`);
      console.log(`[аудит] ${step.name} ✗ ${e.message.split('\n')[0]}`);
      try { await page.screenshot({ path: `${OUT}/${step.name}_FAIL.png` }); } catch {}
      // восстановление: свежая страница на следующем шаге
    }
  }

  console.log('--- ИТОГ ---');
  for (const d of done) console.log(d);
  const errs = logs.filter((l) => l.includes('pageerror@') || l.includes('panic'));
  console.log(`--- ОШИБОК СТРАНИЦЫ: ${errs.length} ---`);
  for (const e of errs.slice(-5)) console.log(e);
  if (ONLY.length) {
    console.log('--- ПОЛНЫЙ КОНСОЛЬ-ЛОГ ---');
    for (const l of logs.slice(-60)) console.log(l);
  }
} finally {
  await browser.close().catch(() => {});
  stop();
}
console.log('AUDIT_DONE');
