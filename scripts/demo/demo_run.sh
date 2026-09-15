#!/bin/bash
# CanvasDesk — визуальное демо под Xvfb: запись GIF/MP4 + скриншоты.
#
# Быстрый запуск (env.sh подключается автоматически):
#   cargo build --release -p canvas-app
#   scripts/demo/demo_run.sh
#
# Свой сценарий (копия demo_driver.py с другими координатами):
#   scripts/demo/demo_run.sh scripts/demo/my_scenario.py
#
# Артефакты: $CANVASDESK_DEMO_OUT (по умолчанию <repo>/demo-out):
#   canvasdesk-demo.gif, canvasdesk-demo.mp4, PNG-скриншоты драйвера.
#
# Переменные: CANVASDESK_APP, DEMO_DISPLAY (:99), DEMO_SIZE (800x600),
#             FPS (15), GIF_FPS (9), CANVASDESK_DEMO_OUT, CANVASDESK_DEMO_BASE.
set -u

DEMO_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DEMO_HOME/../.." && pwd)"

# env.sh идемпотентен — подключаем ВСЕГДА: он использует существующий
# CANVASDESK_DEMO_BASE (если задан) и экспортирует VK/LD_LIBRARY_PATH/
# CANVASDESK_XDOTOOL. Без него приложение упадёт на dlopen libxkbcommon-x11.
# shellcheck disable=SC1091
source "$DEMO_HOME/env.sh"

CANVASDESK_DEMO_OUT="${CANVASDESK_DEMO_OUT:-$REPO_ROOT/demo-out}"
SCRATCH="${CANVASDESK_DEMO_SCRATCH:-$CANVASDESK_DEMO_BASE/run}"
DEMO_DISPLAY="${DEMO_DISPLAY:-:99}"
DEMO_SIZE="${DEMO_SIZE:-800x600}"
FPS="${FPS:-15}"
GIF_FPS="${GIF_FPS:-9}"
APP="${CANVASDESK_APP:-$REPO_ROOT/target/release/canvasdesk}"
DRIVER="${1:-$DEMO_HOME/demo_driver.py}"

[ -x "$APP" ] || { echo "[run] нет бинаря: $APP — сначала 'cargo build --release -p canvas-app'" >&2; exit 1; }
[ -f "$DRIVER" ] || { echo "[run] нет сценария: $DRIVER" >&2; exit 1; }
command -v ffmpeg >/dev/null 2>&1 || { echo "[run] нужен ffmpeg" >&2; exit 1; }
command -v Xvfb >/dev/null 2>&1 || { echo "[run] нужен Xvfb (пакет xvfb)" >&2; exit 1; }

mkdir -p "$SCRATCH" "$CANVASDESK_DEMO_OUT"
cd "$SCRATCH" || exit 1
rm -f default.canvas default.canvas.bak demo_raw.mp4 palette.png
stamp() { echo "[run] $(date +%H:%M:%S) $*"; }

export DISPLAY="$DEMO_DISPLAY"

pkill -f "Xvfb ${DEMO_DISPLAY}" 2>/dev/null; sleep 0.5
# stdout/stderr потомков — не в stdout скрипта, иначе живой потомок
# удерживает пайп и скрипт «висит» после завершения
Xvfb "$DEMO_DISPLAY" -screen 0 "${DEMO_SIZE}x24" -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 1.2
stamp "Xvfb up ($XVFB)"

# Непрерывная запись экрана (lossless-промежуточный; потом из него GIF и MP4)
ffmpeg -y -loglevel error -f x11grab -framerate "$FPS" -video_size "$DEMO_SIZE" -i "$DEMO_DISPLAY" \
  -c:v libx264rgb -preset ultrafast -crf 0 "$SCRATCH/demo_raw.mp4" 2>"$SCRATCH/ffmpeg_rec.log" &
REC=$!
sleep 0.8
stamp "recording started ($REC)"

"$APP" > "$SCRATCH/app.log" 2>&1 &
APP_PID=$!
stamp "app spawned ($APP_PID), waiting for window"

DEMO_DIR="$CANVASDESK_DEMO_OUT" DEMO_DISPLAY="$DEMO_DISPLAY" DEMO_SIZE="$DEMO_SIZE" \
  python3 "$DRIVER"
DRIVER_RC=$?
stamp "driver done rc=$DRIVER_RC"

sleep 1
kill -INT $REC 2>/dev/null; wait $REC 2>/dev/null
if kill -0 $APP_PID 2>/dev/null; then
  kill -TERM $APP_PID 2>/dev/null; sleep 1
  kill -0 $APP_PID 2>/dev/null && kill -KILL $APP_PID 2>/dev/null
fi
kill $XVFB 2>/dev/null
sleep 0.3
kill -KILL $XVFB 2>/dev/null
stamp "teardown done, encoding GIF/MP4"

# GIF: двухпроходная палитра (stats_mode=diff) + dither bayer —
# иначе артефакты на плоских цветах egui
ffmpeg -y -loglevel error -i "$SCRATCH/demo_raw.mp4" \
  -vf "fps=${GIF_FPS},scale=${DEMO_SIZE%%x*}:-1:flags=lanczos,palettegen=stats_mode=diff" \
  "$SCRATCH/palette.png" 2>"$SCRATCH/ffmpeg_gif.log"
ffmpeg -y -loglevel error -i "$SCRATCH/demo_raw.mp4" -i "$SCRATCH/palette.png" \
  -lavfi "fps=${GIF_FPS},scale=${DEMO_SIZE%%x*}:-1:flags=lanczos [x]; [x][1:v] paletteuse=dither=bayer:bayer_scale=4" \
  -loop 0 "$CANVASDESK_DEMO_OUT/canvasdesk-demo.gif"

# MP4: полное разрешение, h264, yuv420p (совместимость с плеерами/мессенджерами)
ffmpeg -y -loglevel error -i "$SCRATCH/demo_raw.mp4" \
  -c:v libx264 -preset medium -crf 22 -pix_fmt yuv420p -movflags +faststart \
  "$CANVASDESK_DEMO_OUT/canvasdesk-demo.mp4" 2>"$SCRATCH/ffmpeg_mp4.log"

stamp "driver_rc=$DRIVER_RC"
echo "[run] --- app.log (tail):"; tail -5 "$SCRATCH/app.log"
echo "[run] --- артефакты:"; ls -la "$CANVASDESK_DEMO_OUT"
exit "$DRIVER_RC"
