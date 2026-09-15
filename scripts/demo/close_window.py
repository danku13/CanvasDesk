#!/usr/bin/env python3
"""Послать WM_DELETE_WINDOW окну из $WINDOW_ID — штатное закрытие
(приложение успевает сохранить канвас). Требует python3-xlib (apt);
при любой ошибке Xlib — резервный путь через `xdotool windowclose`."""
import os
import shutil
import subprocess
import sys

wid = os.environ.get("WINDOW_ID", "").strip()
if not wid:
    sys.exit("WINDOW_ID не задан")


def via_xdotool():
    xdo = os.environ.get("CANVASDESK_XDOTOOL", "xdotool")
    if not shutil.which(xdo):
        sys.exit("нет ни рабочего python3-xlib, ни xdotool — чем закрывать?")
    subprocess.run([xdo, "windowclose", wid])
    print("WM_DELETE sent (xdotool fallback)")


try:
    from Xlib import X, protocol, display
except ImportError:
    via_xdotool()
    sys.exit(0)

try:
    d = display.Display(os.environ.get("DISPLAY", ":99"))
    win = d.create_resource_object("window", int(wid, 0))

    wm_prot = d.intern_atom("WM_PROTOCOLS")
    wm_del = d.intern_atom("WM_DELETE_WINDOW")
    # data = (format=32, [5 long]) — ключевые слова: разные версии python-xlib
    # различаются сигнатурами позиционных аргументов ClientMessage
    ev = protocol.event.ClientMessage(
        window=win.id, client_type=wm_prot,
        data=(32, [wm_del, 0, 0, 0, 0]),
    )
    win.send_event(ev, event_mask=X.NoEventMask)
    d.flush()
    print("WM_DELETE sent")
except Exception as err:  # noqa: BLE001 — резерв на любую версию Xlib/X-ошибку
    print(f"Xlib path failed: {err}", file=sys.stderr)
    via_xdotool()
