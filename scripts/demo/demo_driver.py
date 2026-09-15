#!/usr/bin/env python3
"""CanvasDesk — референсный сценарий визуального демо под Xvfb (Linux).

Запуск вручную не требуется — его вызывает scripts/demo/demo_run.sh.
Свой сценарий = копия этого файла с изменёнными координатами
(см. docs/demo-environment.md, раздел «Адаптация сценария»).

Управление через xdotool (XTEST, устойчив к асинхронным X-ошибкам),
скриншоты через ffmpeg x11grab. Камера по умолчанию (0,0,zoom=1):
screen = world + (400,300).

Сценарий: welcome-сцена → F1 хоткеи → drag ноды → заметки с формулами
(FR-013: 1200 + 480 = 1680) → value-ребро Shift+drag от порта с $in-
формулой (FR-014, live-ревал) → undo/redo → pan/zoom → рамка выделения +
группа → удаление/восстановление → меню канваса и палитра ноды (ПКМ) →
HUD (F3) → поиск (Ctrl+F) → финал.
"""
import os
import subprocess
import sys
import time

DISP = os.environ.get("DEMO_DISPLAY", ":99")
DEMO_DIR = os.environ.get("DEMO_DIR", "./demo-out")
VIDEOSIZE = os.environ.get("DEMO_SIZE", "800x600")
W, H = (int(n) for n in VIDEOSIZE.split("x"))

XDO = os.environ.get("CANVASDESK_XDOTOOL", "xdotool")
XENV = dict(os.environ, DISPLAY=DISP)


def xd(*args):
    cmd = [XDO] + list(args)
    r = subprocess.run(cmd, env=XENV, capture_output=True, text=True)
    if r.returncode != 0:
        print(f"xdotool {args} rc={r.returncode}: {r.stderr.strip()}", file=sys.stderr)
    return r.stdout


def move(x, y):
    xd("mousemove", "--sync", str(int(x)), str(int(y)))


def press(button="1"):
    xd("mousedown", button)


def release(button="1"):
    xd("mouseup", button)


def click(x, y, button="1", clicks=1):
    move(x, y)
    time.sleep(0.06)
    if clicks == 1:
        press(button)
        time.sleep(0.04)
        release(button)
        time.sleep(0.1)
    else:
        xd("click", "--repeat", str(clicks), "--delay", "70", button)
        time.sleep(0.15)


def dblclick(x, y):
    click(x, y, clicks=2)


def key(keys):
    xd("key", "--delay", "40", keys)
    time.sleep(0.08)


def keydown(keysym):
    xd("keydown", keysym)


def keyup(keysym):
    xd("keyup", keysym)


def type_str(s):
    xd("type", "--delay", "35", s)
    time.sleep(0.1)


def drag(x1, y1, button="1", shift=False, steps=14):
    """Drag от текущей позиции курсора к (x1, y1)."""
    if shift:
        keydown("shift")
        time.sleep(0.06)
    press(button)
    time.sleep(0.08)
    # промежуточные точки — приложение видит плавное движение
    cx, cy = get_cursor()
    for i in range(1, steps + 1):
        xx = round(cx + (x1 - cx) * i / steps)
        yy = round(cy + (y1 - cy) * i / steps)
        xd("mousemove", str(xx), str(yy))
        time.sleep(0.02)
    time.sleep(0.12)
    release(button)
    if shift:
        time.sleep(0.06)
        keyup("shift")
    time.sleep(0.1)


def get_cursor():
    out = xd("getmouselocation", "--shell")
    vals = dict(line.split("=", 1) for line in out.splitlines() if "=" in line)
    return int(vals.get("X", 0)), int(vals.get("Y", 0))


def pan(x1, y1):
    keydown("space")
    time.sleep(0.12)
    drag(x1, y1)
    keyup("space")


def ctrl_key(ch):
    key(f"ctrl+{ch}")


def wheel(n, up=True, ctrl=False):
    btn = "4" if up else "5"
    if ctrl:
        keydown("ctrl")
        time.sleep(0.05)
    for _ in range(n):
        xd("click", btn)
        time.sleep(0.12)
    if ctrl:
        time.sleep(0.05)
        keyup("ctrl")


def shot(name, dwell=0.0):
    if dwell:
        time.sleep(dwell)
    path = os.path.join(DEMO_DIR, name)
    env = dict(os.environ, DISPLAY=DISP)
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error",
         "-f", "x11grab", "-video_size", VIDEOSIZE, "-i", DISP,
         "-frames:v", "1", path],
        check=True, env=env,
    )
    print(f"shot: {name}", flush=True)


def wait_window(timeout=30):
    t0 = time.time()
    while time.time() - t0 < timeout:
        wid = xd("search", "--onlyvisible", "--name", "CanvasDesk").strip()
        if wid:
            lines = [ln for ln in wid.splitlines() if ln.strip()]
            if lines:
                print(f"window found: {lines[0]}", flush=True)
                xd("windowfocus", lines[0])
                return True
        time.sleep(0.4)
    return False


def close_gracefully():
    """WM_DELETE_WINDOW — штатное закрытие (приложение сохраняет канвас)."""
    wid = xd("search", "--onlyvisible", "--name", "CanvasDesk").strip().splitlines()
    if not wid:
        return
    env = dict(os.environ, DISPLAY=DISP, WINDOW_ID=wid[0])
    closer = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                          "close_window.py")
    subprocess.run(["python3", closer], env=env)


# --- сценарий ---------------------------------------------------------------

assert wait_window(), "окно CanvasDesk не найдено"
time.sleep(4.0)  # прогрев wgpu/lavapipe, стартовые кадры

# 01: стартовая сцена (welcome-канвас: заметка + 2 файл-карточки)
click(W // 2, H // 2)
shot("01-launch.png", dwell=0.4)

# 02: оверлей горячих клавиш (F1)
key("F1")
shot("02-hotkeys.png", dwell=1.0)
time.sleep(2.0)
key("Escape")
time.sleep(0.6)

# 03: перетаскивание welcome-заметки в левый верх (drag ноды)
move(620, 420)
drag(200, 150)
time.sleep(0.6)

# 04: заметка-источник с формулой (FR-013): 1200 + 480 = 1680
#     world (50,30); порт Right = world (310,90) → screen (710,390)
dblclick(450, 330)
time.sleep(0.5)
type_str("1200 + 480")
key("Return")
time.sleep(0.8)

# 05: заметка-получатель ($in — вход value-потока, FR-014)
#     world (50,-170); порт Left = world (50,-110) → screen (450,190)
dblclick(450, 130)
time.sleep(0.5)
type_str("$in / 3")
key("Return")
time.sleep(0.8)

# 06: value-ребро Shift+drag от порта A.right к порту B.left (FR-014)
# press/drop — чуть ВНУТРИ нод (на границе spatial hit не находит ноду)
move(705, 390)
drag(455, 190, shift=True, steps=22)
shot("03-value-flow.png", dwell=1.2)
time.sleep(2.2)

# 07: undo/redo ребра (контроль undo-стека, мигание в GIF)
ctrl_key("z")
time.sleep(0.9)
ctrl_key("y")
time.sleep(1.1)

# 08: панорамирование Space+drag; масштаб Ctrl+колесо (туда-обратно)
move(400, 300)
pan(250, 300)
time.sleep(0.5)
wheel(2, up=True, ctrl=True)
time.sleep(0.6)
wheel(2, up=False, ctrl=True)
time.sleep(0.5)
move(250, 300)
pan(400, 300)
time.sleep(0.6)

# 09: рамка выделения + группа (Ctrl+G) — все три ноды
move(40, 70)
drag(740, 470, steps=18)
time.sleep(0.4)
ctrl_key("g")
shot("04-group.png", dwell=0.9)
time.sleep(1.4)
ctrl_key("z")
time.sleep(0.8)

# 10: удаление выделенной ноды (каскад со связями) + восстановление
click(560, 390)
time.sleep(0.4)
key("Delete")
time.sleep(0.9)
ctrl_key("z")
time.sleep(1.0)

# 11а: меню пустого канваса (ПКМ по свободному месту)
click(700, 550, button="3")
shot("06-canvas-menu.png", dwell=0.9)
time.sleep(1.8)
key("Escape")
time.sleep(0.5)

# 11б: контекстная палитра ноды (ПКМ по welcome-заметке, центр (200,150))
click(200, 150, button="3")
shot("06-node-palette.png", dwell=0.9)
time.sleep(1.8)
key("Escape")
time.sleep(0.5)

# 12: HUD (F3) — fps/p95 кадра. ВАЖНО: до поиска (Ctrl+F), иначе при
# сохранённых строках поиска F3 циклит результаты, а не показывает HUD
key("F3")
shot("07-hud.png", dwell=0.8)
time.sleep(1.4)
key("F3")
time.sleep(0.5)

# 13: поиск по канвасу (Ctrl+F) — запрос по формуле
ctrl_key("f")
time.sleep(0.4)
type_str("1200")
shot("05-search.png", dwell=0.9)
time.sleep(1.6)
key("Escape")
time.sleep(0.5)

# 14: финальный план сцены (zoom out)
wheel(2, up=False, ctrl=True)
time.sleep(0.8)
shot("08-final.png", dwell=0.4)
time.sleep(1.0)

close_gracefully()
time.sleep(2.0)
print("driver done", flush=True)
