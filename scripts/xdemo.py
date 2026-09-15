#!/usr/bin/env python3
"""Автоматизация ввода для демо CanvasDesk под Xvfb (XTEST через python-xlib).

Субкоманды:
  move X Y            — переместить указатель
  click X Y [N]       — кликнуть (N=2 — двойной клик) лево кнопкой
  key KEYSYM          — нажать и отпустить клавишу (например Return, End)
  type "TEXT"         — напечатать текст (latin1-символы, Shift для верхнего ряда)
  drag X1 Y1 X2 Y2    — нажать, перенести, отпустить
"""
import sys
import time

from Xlib import X, display, XK
from Xlib.ext import xtest

DELAY = 0.012  # пауза между нажатиями — «живая» печать


def get_display():
    d = display.Display()
    return d


def keysym_for(ch: str):
    special = {
        '\n': 'Return', '\t': 'Tab', ' ': 'space',
    }
    if ch in special:
        return XK.string_to_keysym(special[ch])
    # « печатаемый ascii » — прямое имя; верхний ряд — через shift
    if ch.isalnum() and ch.isascii():
        return XK.string_to_keysym(ch.lower())
    named = {
        '=': 'equal', '+': 'plus', '-': 'minus', '/': 'slash',
        '.': 'period', ',': 'comma', '*': 'asterisk', '(': 'parenleft',
        ')': 'parenright', ':': 'colon', '_': 'underscore', '$': 'dollar',
        '%': 'percent', '!': 'exclam', '×': 'multiply', '←': 'Left',
    }
    if ch in named:
        return XK.string_to_keysym(named[ch])
    return None


SHIFTED_SYMS = set()
for s in ('plus', 'asterisk', 'underscore', 'dollar', 'percent', 'exclam',
          'parenleft', 'parenright', 'colon'):
    SHIFTED_SYMS.add(XK.string_to_keysym(s))


def press(d, keycode):
    xtest.fake_input(d, X.KeyPress, keycode)
    d.sync()


def release(d, keycode):
    xtest.fake_input(d, X.KeyRelease, keycode)
    d.sync()


def tap_key(d, keysym, delay=DELAY, force_shift=False):
    kc = d.keysym_to_keycode(keysym)
    if kc == 0:
        print(f"нет keycode для keysym {keysym}", file=sys.stderr)
        return
    need_shift = force_shift or keysym in SHIFTED_SYMS
    shift_kc = d.keysym_to_keycode(XK.string_to_keysym('Shift_L'))
    if need_shift:
        press(d, shift_kc)
        time.sleep(0.01)
    press(d, kc)
    time.sleep(delay)
    release(d, kc)
    if need_shift:
        time.sleep(0.01)
        release(d, shift_kc)
    d.sync()


def click(d, x, y, count=1):
    d.screen().root.warp_pointer(x, y)
    d.sync()
    time.sleep(0.05)
    for _ in range(count):
        xtest.fake_input(d, X.ButtonPress, 1)
        d.sync()
        time.sleep(0.04)
        xtest.fake_input(d, X.ButtonRelease, 1)
        d.sync()
        time.sleep(0.09 if count > 1 else 0.05)


def type_text(d, text):
    import os
    char_delay = float(os.environ.get('XDEMO_CHAR_DELAY', DELAY))
    line_delay = float(os.environ.get('XDEMO_LINE_DELAY', 0.4))
    ret = XK.string_to_keysym('Return')
    for ch in text:
        if ch == '\n':
            # Shift+Enter — новая строка (Enter без Shift = commit)
            tap_key(d, ret, delay=0.04, force_shift=True)
            time.sleep(line_delay)
        else:
            ks = keysym_for(ch)
            if ks is None:
                continue
            tap_key(d, ks)
            time.sleep(char_delay)
    d.sync()


def find_app_window(d):
    """Найти окно CanvasDesk (дочернее окно корня, отличное от 1x1)."""
    root = d.screen().root
    stack = [root]
    while stack:
        w = stack.pop()
        try:
            geom = w.get_geometry()
        except Exception:
            continue
        if w.id != root.id and geom.width > 100 and geom.height > 100:
            name = ''
            try:
                name = w.get_wm_name() or ''
            except Exception:
                pass
            if 'CanvasDesk' in str(name) or (geom.width == 800 and geom.height == 600):
                return w
        try:
            stack.extend(w.query_tree().children)
        except Exception:
            pass
    return None


def main():
    d = get_display()
    cmd = sys.argv[1]
    if cmd == 'focus':
        w = find_app_window(d)
        if w is None:
            print('окно CanvasDesk не найдено', file=sys.stderr)
            sys.exit(2)
        w.set_input_focus(X.RevertToParent, X.CurrentTime)
        d.sync()
        time.sleep(0.2)
        print('focus set')
    elif cmd == 'resize':
        w = find_app_window(d)
        if w is None:
            print('окно CanvasDesk не найдено', file=sys.stderr)
            sys.exit(2)
        w.configure(width=int(sys.argv[2]), height=int(sys.argv[3]),
                    x=int(sys.argv[4]) if len(sys.argv) > 4 else 0,
                    y=int(sys.argv[5]) if len(sys.argv) > 5 else 0)
        d.sync()
        time.sleep(0.4)
        print(f'resized: {w.get_geometry().width}x{w.get_geometry().height}')
    elif cmd == 'move':
        d.screen().root.warp_pointer(int(sys.argv[2]), int(sys.argv[3]))
        d.sync()
    elif cmd == 'click':
        x, y = int(sys.argv[2]), int(sys.argv[3])
        n = int(sys.argv[4]) if len(sys.argv) > 4 else 1
        click(d, x, y, n)
    elif cmd == 'key':
        tap_key(d, XK.string_to_keysym(sys.argv[2]), delay=0.03)
    elif cmd == 'type':
        type_text(d, sys.argv[2])
    elif cmd == 'wheel':
        # wheel X Y N [ctrl] — N «щелчков» колеса вверх (N<0 — вниз);
        # ctrl=1 — зажать Control (Ctrl+колесо = зум к курсору)
        x, y = int(sys.argv[2]), int(sys.argv[3])
        n = int(sys.argv[4])
        with_ctrl = len(sys.argv) > 5 and sys.argv[5] == '1'
        d.screen().root.warp_pointer(x, y)
        d.sync()
        time.sleep(0.1)
        ctrl_kc = d.keysym_to_keycode(XK.string_to_keysym('Control_L'))
        if with_ctrl:
            press(d, ctrl_kc)
            time.sleep(0.05)
        btn = 4 if n > 0 else 5
        for _ in range(abs(n)):
            xtest.fake_input(d, X.ButtonPress, btn)
            d.sync()
            time.sleep(0.04)
            xtest.fake_input(d, X.ButtonRelease, btn)
            d.sync()
            time.sleep(0.12)
        if with_ctrl:
            release(d, ctrl_kc)
        d.sync()
    elif cmd == 'drag':
        x1, y1, x2, y2 = map(int, sys.argv[2:6])
        btn = int(sys.argv[6]) if len(sys.argv) > 6 else 1
        d.screen().root.warp_pointer(x1, y1)
        d.sync()
        time.sleep(0.05)
        xtest.fake_input(d, X.ButtonPress, btn)
        d.sync()
        steps = 12
        for i in range(1, steps + 1):
            cx = x1 + (x2 - x1) * i // steps
            cy = y1 + (y2 - y1) * i // steps
            d.screen().root.warp_pointer(cx, cy)
            d.sync()
            time.sleep(0.02)
        xtest.fake_input(d, X.ButtonRelease, btn)
        d.sync()
    else:
        print(__doc__)
        sys.exit(1)


if __name__ == '__main__':
    main()
