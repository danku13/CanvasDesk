#!/usr/bin/env bash
# Сборка macOS-дистрибутива CanvasDesk (CI-гейт, запускается на macos-latest).
#
# Проблема, которую решает: голый бинарник из артефакта не запускается на
# Mac пользователя — (а) без exec-бита после распаковки zip, (б) с карантином
# com.apple.quarantine Gatekeeper блокирует неподписанный файл, (в) macos-latest
# даёт только arm64 (Intel-Мак не запустит), (г) сырой бинарник Finder не
# запускает двойным кликом.
#
# Что делает скрипт:
#   1. lipo -create: arm64 + x86_64 слайсы -> universal2-бинарник;
#   2. CanvasDesk.app (Contents/MacOS/canvasdesk + Info.plist) — нормальный
#      запуск двойным кликом из Finder;
#   3. ad-hoc codesign (-s -): убирает ошибку «файл повреждён» у неподписанного
#      приложения (полноценная подпись требует Apple Developer ID — не CI-гейт);
#   4. LAUNCH-macOS.txt: инструкция снятия карантина (xattr) и запуска.
#
# Вход: target/{aarch64,x86_64}-apple-darwin/release/canvasdesk (обе сборки
# делает workflow до вызова). Выход: dist/ (CanvasDesk.app, canvasdesk,
# LAUNCH-macOS.txt). tar-упаковку делает шаг workflow (см. ci.yml).
set -euo pipefail

cd "$(dirname "$0")/.." # корень репозитория

ARM=target/aarch64-apple-darwin/release/canvasdesk
X86=target/x86_64-apple-darwin/release/canvasdesk

for bin in "$ARM" "$X86"; do
    if [ ! -x "$bin" ]; then
        echo "ошибка: нет бинарника $bin (соберите оба таргета перед пакеджингом)" >&2
        exit 1
    fi
done

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
echo "версия дистрибутива: $VERSION"

mkdir -p dist
rm -rf dist/CanvasDesk.app dist/canvasdesk dist/LAUNCH-macOS.txt

# ---- 1. universal2 -----------------------------------------------------
lipo -create -output dist/canvasdesk "$ARM" "$X86"
chmod +x dist/canvasdesk
lipo -info dist/canvasdesk
file dist/canvasdesk

# ---- 2. CanvasDesk.app -------------------------------------------------
APP=dist/CanvasDesk.app
mkdir -p "$APP/Contents/MacOS"
cp dist/canvasdesk "$APP/Contents/MacOS/canvasdesk"
chmod +x "$APP/Contents/MacOS/canvasdesk"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>CanvasDesk</string>
    <key>CFBundleDisplayName</key>
    <string>CanvasDesk</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.danku13.CanvasDesk</string>
    <key>CFBundleExecutable</key>
    <string>canvasdesk</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.productivity</string>
</dict>
</plist>
PLIST

# ---- 3. ad-hoc подпись -------------------------------------------------
# -s - : ad-hoc (без Developer ID). Гасит «приложение повреждено» при
# переносе; карантин всё равно снимается xattr (инструкция в LAUNCH).
codesign --force -s - "$APP"
codesign --verify --verbose=1 "$APP"
codesign -dv "$APP" 2>&1 | head -5 || true

# ---- 4. инструкция запуска ---------------------------------------------
cat > dist/LAUNCH-macOS.txt <<LAUNCH
CanvasDesk ${VERSION} для macOS — universal2 (Apple Silicon + Intel)

Почему нужен шаг 2: приложение собрано в CI без платной подписи Apple
Developer ID, а macOS ставит «карантин» на всё скачанное из сети —
без снятия Gatekeeper покажет «не удаётся открыть» / «повреждено».

Установка и запуск:
  1) Распакуйте архив (двойной клик по .tar.gz).
  2) Снимите карантин в Терминале:
       xattr -cr CanvasDesk.app
  3) Запустите двойным кликом по CanvasDesk.app.
     Если macOS всё равно предупреждает: правый клик по приложению ->
     «Открыть» -> «Открыть», либо Системные настройки -> Конфиденциальность
     и безопасность -> «Открыть всё равно».

Запуск из Терминала (без .app):
  ./canvasdesk

Флаги: --stress N — сцена из N случайных нод; --desktop — режим обоев
(только Windows). Данные: ~/.canvasdesk/ (сцена default.canvas, конфиг).

Проверка сборки: lipo -info canvasdesk (ожидается x86_64 + arm64).
LAUNCH

echo "готово: dist/CanvasDesk.app, dist/canvasdesk, dist/LAUNCH-macOS.txt"
