#!/bin/bash
# Запуск canvasdesk под Xvfb + скриншоты (ffmpeg x11grab)
source /home/z/.cargo/env
export DISPLAY=:77
export LD_LIBRARY_PATH=/home/z/my-project/deps/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH
export WGPU_BACKEND=gl
export LIBGL_ALWAYS_SOFTWARE=1
export __EGL_VENDOR_LIBRARY_FILENAMES=/home/z/my-project/deps/usr/share/glvnd/egl_vendor.d/50_mesa.json
export LIBGL_DRIVERS_PATH=/home/z/my-project/deps/usr/lib/x86_64-linux-gnu/dri
export MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
Xvfb :77 -screen 0 1600x900x24 &
XVFB_PID=$!
sleep 1.5
cd /home/z/my-project/canvasdesk
target/debug/canvasdesk > /home/z/my-project/shots/app.log 2>&1 &
APP_PID=$!
sleep 12
ffmpeg -y -f x11grab -video_size 1600x900 -i :77 -frames:v 1 /home/z/my-project/shots/shot_boot.png -loglevel error
sleep 4
ffmpeg -y -f x11grab -video_size 1600x900 -i :77 -frames:v 1 /home/z/my-project/shots/shot_late.png -loglevel error
kill $APP_PID $XVFB_PID 2>/dev/null
wait 2>/dev/null
echo "done"
