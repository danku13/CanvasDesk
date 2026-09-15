#!/bin/bash
# Bootstrap окружения визуального демо CanvasDesk (Linux, headless, без root).
#
# Использование (идемпотентно — можно вызывать каждый раз):
#   source scripts/demo/env.sh
#
# Что делает:
#   1. Проверяет наличие библиотек (системных и уже извлечённых в кэш).
#   2. Докачивает недостающие .deb (apt-get download) и распаковывает их
#      через dpkg -x в $CANVASDESK_DEMO_BASE — без установки в систему.
#   3. Экспортирует переменные окружения для wgpu/lavapipe и xdotool.
#
# Кэш: ~/.cache/canvasdesk-demo/{debs,mesa-vk,syslibs} (переопределяется
# переменной CANVASDESK_DEMO_BASE).

CANVASDESK_DEMO_BASE="${CANVASDESK_DEMO_BASE:-$HOME/.cache/canvasdesk-demo}"
DEB_DIR="$CANVASDESK_DEMO_BASE/debs"
MESA_ROOT="$CANVASDESK_DEMO_BASE/mesa-vk"
SYS_ROOT="$CANVASDESK_DEMO_BASE/syslibs"

have_lib() { ldconfig -p 2>/dev/null | grep -q "$1"; }
syslib_extracted() { ls "$SYS_ROOT"/usr/lib/x86_64-linux-gnu/"$1".so* >/dev/null 2>&1; }

demo_dev_symlinks() {
  # dlopen-обёртки (xkbcommon-dl и др.) могут запрашивать безверсионные
  # имена lib*.so — добавляем symlink на старшую версию, если его нет.
  local dir so base
  for dir in "$MESA_ROOT"/usr/lib/x86_64-linux-gnu "$SYS_ROOT"/usr/lib/x86_64-linux-gnu; do
    [ -d "$dir" ] || continue
    for so in "$dir"/lib*.so.[0-9]*; do
      [ -e "$so" ] || continue
      base="${so%.so.*}.so"
      [ -e "$base" ] || ln -s "$(basename "$so")" "$base"
    done
  done
}

demo_bootstrap() {
  local need=()
  mkdir -p "$DEB_DIR" "$MESA_ROOT" "$SYS_ROOT"

  # lavapipe — софтверный Vulkan ICD (обязателен: GPU в контейнере/CI нет)
  if [ ! -f "$MESA_ROOT/usr/share/vulkan/icd.d/lvp_icd.json" ]; then
    need+=(mesa-vulkan-drivers)
  fi
  # загрузчик Vulkan (обычно уже есть в базовом образе)
  if ! have_lib "libvulkan.so.1" && ! syslib_extracted "libvulkan"; then
    need+=(libvulkan1)
  fi
  # клавиатура winit под X11 + xcb-xkb
  if ! have_lib "libxkbcommon-x11.so.0" && ! syslib_extracted "libxkbcommon-x11"; then
    need+=(libxkbcommon-x11-0)
  fi
  if ! have_lib "libxcb-xkb.so.1" && ! syslib_extracted "libxcb-xkb"; then
    need+=(libxcb-xkb1)
  fi
  # xdotool — XTEST-драйвер сценария (libxdo3/libxtst6 — его зависимости)
  if ! command -v xdotool >/dev/null 2>&1 && [ ! -x "$SYS_ROOT/usr/bin/xdotool" ]; then
    need+=(xdotool libxdo3 libxtst6)
  fi

  if [ "${#need[@]}" -gt 0 ]; then
    echo "[env] докачиваю пакеты: ${need[*]}"
    ( cd "$DEB_DIR" && apt-get download "${need[@]}" ) || {
      echo "[env] ОШИБКА: apt-get download не сработал." >&2
      echo "[env] Варианты: 1) один раз выполнить 'sudo apt-get update';" >&2
      echo "[env] 2) скачать .deb вручную (packages.ubuntu.com) в $DEB_DIR и перезапустить." >&2
      return 1
    }
    local deb
    for deb in "$DEB_DIR"/*.deb; do
      case "$(basename "$deb")" in
        mesa-vulkan-drivers*) dpkg -x "$deb" "$MESA_ROOT" ;;
        *)                    dpkg -x "$deb" "$SYS_ROOT" ;;
      esac
    done
  else
    echo "[env] зависимости уже на месте"
  fi
  demo_dev_symlinks
}

demo_export_env() {
  # lavapipe ICD: VK_DRIVER_FILES (новое имя) + VK_ICD_FILENAMES (алиас)
  if [ -f "$MESA_ROOT/usr/share/vulkan/icd.d/lvp_icd.json" ]; then
    export VK_DRIVER_FILES="$MESA_ROOT/usr/share/vulkan/icd.d/lvp_icd.json"
    export VK_ICD_FILENAMES="$VK_DRIVER_FILES"
  fi
  local libdirs="$MESA_ROOT/usr/lib/x86_64-linux-gnu:$SYS_ROOT/usr/lib/x86_64-linux-gnu"
  export LD_LIBRARY_PATH="$libdirs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  # спам wgpu-логов в контейнер не нужен
  export RUST_LOG="${RUST_LOG:-info,wgpu_hal=warn,wgpu_core=warn}"
  # путь к xdotool (из кэша или PATH) — его читает demo_driver.py
  if command -v xdotool >/dev/null 2>&1; then
    export CANVASDESK_XDOTOOL="$(command -v xdotool)"
  elif [ -x "$SYS_ROOT/usr/bin/xdotool" ]; then
    export CANVASDESK_XDOTOOL="$SYS_ROOT/usr/bin/xdotool"
  fi
}

demo_bootstrap || return 1 2>/dev/null || exit 1
demo_export_env

echo "[env] CANVASDESK_DEMO_BASE=$CANVASDESK_DEMO_BASE"
echo "[env] VK_DRIVER_FILES=${VK_DRIVER_FILES:-<системный>}"
echo "[env] CANVASDESK_XDOTOOL=${CANVASDESK_XDOTOOL:-<не найден!>}"
