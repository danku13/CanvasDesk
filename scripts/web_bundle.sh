#!/usr/bin/env bash
# M8/W12 (wasm-port §6, задача W12 «Полировка и деплой»): релизный web-бандл
# canvas-web одной командой — сборка, оптимизация, отчёт о размере.
#
#   1/3 build:  trunk build --release (профиль workspace: [profile.release]
#               lto = "thin" — W12; public-url настраивается для хостинга)
#   2/3 opt:    wasm-opt -Oz (binaryen) поверх каждого .wasm — основное
#               сжатие бандла; нет binaryen — предупреждение, бандл без
#               оптимизации (деплой-CI ставит его сам)
#   3/3 report: raw/brotli размеры по файлам и итог; цель §8.8 — бандл
#               ≤8 МБ raw / ≤4 МБ brotli, оценка загрузки на 25 Мбит/с
#               (§8 «деплой грузится ≤5 с»); в CI итог дублируется в
#               $GITHUB_STEP_SUMMARY (размер бандла в CI-логе — W12)
#
# Использование:
#   scripts/web_bundle.sh                                           # локально: public-url /
#   scripts/web_bundle.sh --public-url /CanvasDesk/app/ --dist _site/app   # деплой-CI (Pages /app)
#   scripts/web_bundle.sh --report-only                             # только отчёт по готовому бандлу
#
# Инструменты (версии зафиксированы — Trunk.toml, воспроизводимость):
#   trunk 0.21.14, wasm-bindgen-cli 0.2.127 (совпадает с Cargo.lock;
#   trunk скачает его сам, при 404 — cargo install wasm-bindgen-cli
#   --version 0.2.127 --locked), binaryen/wasm-opt — любой свежий.
set -euo pipefail
cd "$(dirname "$0")/.."

PUBLIC_URL="/"
DIST="target/dist"
REPORT_ONLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --public-url) PUBLIC_URL="$2"; shift 2 ;;
        --dist) DIST="$2"; shift 2 ;;
        --report-only) REPORT_ONLY=1; shift ;;
        *) echo "неизвестный аргумент: $1 (доступны: --public-url URL, --dist DIR, --report-only)" >&2; exit 2 ;;
    esac
done

# Trunk.toml: dist = ../../target/dist — внутри trunk всегда дефолтный путь,
# копирование в $DIST отдельным шагом (аргумент --dist trunk относителен к
# cwd, его смешение с Trunk.toml-путями — источник ошибок).
BUNDLE="target/dist"

if [ "$REPORT_ONLY" -eq 0 ]; then
    command -v trunk >/dev/null 2>&1 || {
        echo "trunk не найден. Установка: cargo install trunk --version 0.21.14 --locked" >&2
        exit 2
    }

    echo "[web-bundle 1/3] trunk build --release (public-url: $PUBLIC_URL)"
    (cd crates/canvas-web && NO_COLOR=false trunk build --release --public-url "$PUBLIC_URL")
    # public-url переписывает пути ассетов в index.html: локально «/»,
    # Pages-деплой — база сайта репозитория (см. pages-web.yml).

    echo "[web-bundle 2/3] wasm-opt -Oz"
    if command -v wasm-opt >/dev/null 2>&1; then
        shopt -s nullglob
        for wasm in "$BUNDLE"/*.wasm; do
            before=$(stat -c%s "$wasm")
            wasm-opt -Oz "$wasm" -o "$wasm.opt" && mv "$wasm.opt" "$wasm"
            after=$(stat -c%s "$wasm")
            printf '  %s: %d -> %d байт (-%d%%)\n' "$(basename "$wasm")" "$before" "$after" "$(( (before - after) * 100 / before ))"
        done
        shopt -u nullglob
    else
        echo "  wasm-opt (binaryen) не найден — бандл без -Oz-оптимизации" >&2
        echo "  установка: https://github.com/WebAssembly/binaryen/releases (деплой-CI ставит сам)" >&2
    fi
fi

echo "[web-bundle 3/3] отчёт о размере (§8.8: ≤8 МБ raw / ≤4 МБ brotli)"
compress() { # brotli при наличии, иначе gzip -9 (оценка сверху)
    if command -v brotli >/dev/null 2>&1; then
        brotli -q 11 -c "$1" 2>/dev/null | wc -c
    else
        gzip -9 -c "$1" | wc -c
    fi
}
COMPRESSOR=$(command -v brotli >/dev/null 2>&1 && echo brotli || echo gzip)

TOTAL_RAW=0
TOTAL_CMP=0
printf '  %-34s %12s %12s\n' 'файл' 'raw' "$COMPRESSOR"
for f in "$BUNDLE"/*; do
    [ -f "$f" ] || continue
    raw=$(stat -c%s "$f")
    cmp_=$(compress "$f")
    TOTAL_RAW=$((TOTAL_RAW + raw))
    TOTAL_CMP=$((TOTAL_CMP + cmp_))
    printf '  %-34s %12d %12d\n' "$(basename "$f")" "$raw" "$cmp_"
done
MB=$((1024 * 1024))
printf '  %-34s %12d %12d (байт)\n' 'ИТОГО' "$TOTAL_RAW" "$TOTAL_CMP"
printf '  итог: %d.%02d МБ raw / %d.%02d МБ %s\n' \
    $((TOTAL_RAW / MB)) $(((TOTAL_RAW % MB) * 100 / MB)) \
    $((TOTAL_CMP / MB)) $(((TOTAL_CMP % MB) * 100 / MB)) "$COMPRESSOR"

# Оценка загрузки §8 «деплой грузится ≤5 с» (25 Мбит/с = 3.125 МБ/с):
SECONDS_EST=$(( (TOTAL_CMP * 8 + 24999999) / 25000000 ))
printf '  оценка загрузки (25 Мбит/с): ~%d с (лимит §8: 5 с)\n' "$SECONDS_EST"

STATUS="OK"
if [ "$TOTAL_RAW" -gt $((8 * MB)) ]; then
    echo "  ПРЕВЫШЕН лимит raw 8 МБ (§8.8)" >&2
    STATUS="FAIL-raw"
fi
if [ "$TOTAL_CMP" -gt $((4 * MB)) ]; then
    echo "  ПРЕВЫШЕН лимит $COMPRESSOR 4 МБ (§8.8; gzip-оценка консервативнее brotli)" >&2
    STATUS="FAIL-compressed"
fi
echo "[web-bundle] размер: $STATUS"

# В CI дублируем итог в summary пуша («бандл-размер в CI-логе», W12).
if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f "${GITHUB_STEP_SUMMARY}" ]; then
    {
        echo "### Web-бандл canvas-web (${PUBLIC_URL})"
        echo ""
        echo "- raw: $((TOTAL_RAW / MB)).$(((TOTAL_RAW % MB) * 100 / MB)) МБ (лимит §8.8: 8 МБ)"
        echo "- $COMPRESSOR: $((TOTAL_CMP / MB)).$(((TOTAL_CMP % MB) * 100 / MB)) МБ (лимит §8.8: 4 МБ)"
        echo "- оценка загрузки на 25 Мбит/с: ~${SECONDS_EST} с (лимит: 5 с)"
        echo "- статус: $STATUS"
    } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$DIST" != "$BUNDLE" ]; then
    mkdir -p "$DIST"
    cp -a "$BUNDLE"/. "$DIST"/
    echo "[web-bundle] скопировано в $DIST"
fi
echo "[web-bundle] OK"
