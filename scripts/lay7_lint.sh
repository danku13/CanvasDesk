#!/usr/bin/env bash
# LAY-W12 (аудит layouts-2026-10 §5): регрессионный гейт LAY7 —
# литеральные зазоры и ручной курсор y+= в canvas-app.
#
# Ловит НОВЫЕ нарушения LAY7 (существующие — в allowlist до волны W3/W6):
# 1. gap: <литерал float> в вызовах Row/Column — кроме 0.0 (нейтральный
#    ритм) и SPACING_*/именованных констант (токен-референс);
#    collision_gap/row_gap — НЕ LAY7 (collision/snap-параметры), исключаются;
# 2. y += <литерал float> — ручной курсор (анти-паттерн LAY2/LAY10),
#    вне allowlist файлов (kit_ui, admin_ui, overlays, stage — W3).
#
# Allowlist для y +=: 4 файла со «скелетами» ручной раскладки (аудит §3.7).
# После волны W3 (миграция на Column-примитивы) allowlist удаляется.
#
# Запуск: bash scripts/lay7_lint.sh
# Выход: 0 — новых нарушений нет; 1 — найдены (см. вывод).
set -euo pipefail
cd "$(dirname "$0")/.."

# Allowlist: файлы где y += <literal> разрешён до волны W3
# (аудит §3.7 — ручные скелеты админки/витрины/оверлеев/stage:
#  kit_ui.rs — 36 вхождений, admin_ui.rs — 17, app/overlays.rs — 2,
#  app/stage.rs — 1; итого 56 — все на миграцию в W3).
ALLOW_YPLUS_REGEX='/(kit_ui|admin_ui|overlays|stage)\.rs$'

violations=0

# --- 1. gap: <литерал float> в Row/Column (кроме 0.0, collision_gap, row_gap) ---
echo "--- LAY7: gap literals ---"
gap_hits=""
while IFS= read -r f; do
  # awk: отрезать всё после первого #[cfg(test)] (тесты — в конце файла);
  # префикс FILENAME:FNR: — для удобной локализации находок.
  # grep -vE 1: выкинуть doc/line-комментарии (///, //, //!).
  # rg: литерал вида gap: <digit>.<digit> (не 0.0 и не именованная константа).
  # grep -vE 2: collision_gap/row_gap — НЕ LAY7 (collision/snap-параметры).
  # grep -vE 3: gap: 0.* — нейтральный «ритм без зазора» (аудит §3.7 P3).
  hits=$(awk '/#\[cfg\(test\)\]/{exit} {print FILENAME ":" FNR ":" $0}' "$f" \
    | grep -vE '^\S+:[0-9]+:[[:space:]]*(///|//!|//)' \
    | rg 'gap:[[:space:]]*[0-9]+\.[0-9]+' \
    | grep -vE 'collision_gap:|row_gap:' \
    | grep -vE 'gap:[[:space:]]*0\.' || true)
  gap_hits="${gap_hits}${hits}"
done < <(git ls-files 'crates/canvas-app/src/*.rs')

if [ -n "$gap_hits" ]; then
  echo "FAIL: найдены литеральные gap значения (должны быть 0.0 или SPACING_*):"
  echo "$gap_hits"
  violations=1
else
  echo "OK — литеральных gap вне 0.0/SPACING_* нет"
fi

# --- 2. y += <литерал float> вне allowlist (анти-паттерн LAY2/LAY10) ---
echo "--- LAY7: y += literal (outside allowlist) ---"
yplus_hits=""
while IFS= read -r f; do
  # Пропустить allowlist файлы (W3 — миграция скелетов на Column-примитивы).
  if echo "$f" | grep -qE "$ALLOW_YPLUS_REGEX"; then continue; fi
  # Паттерн ловит y, py, ext_y, *y — любые переменные, оканчивающиеся на y,
  # с инкрементом на литерал float. Совпадает с перечнем §3.7-P3 аудита.
  hits=$(awk '/#\[cfg\(test\)\]/{exit} {print FILENAME ":" FNR ":" $0}' "$f" \
    | grep -vE '^\S+:[0-9]+:[[:space:]]*(///|//!|//)' \
    | rg 'y[[:space:]]*\+=[[:space:]]*[0-9]+\.[0-9]+' || true)
  yplus_hits="${yplus_hits}${hits}"
done < <(git ls-files 'crates/canvas-app/src/*.rs')

if [ -n "$yplus_hits" ]; then
  echo "FAIL: найден ручной курсор y += <literal> вне allowlist:"
  echo "$yplus_hits"
  violations=1
else
  echo "OK — y += <literal> вне allowlist нет"
fi

# --- Итог ---
if [ "$violations" -ne 0 ]; then
  echo
  echo "LAY7 lint: FAIL (см. выше)"
  echo "Allowlist файлов для y +=: kit_ui, admin_ui, overlays, stage (до волны W3)"
  echo "См. design/layouts-audit-2026-10.md §3.7 и §5 (LAY-W12)."
  exit 1
fi

echo
echo "LAY7 lint: OK — новых нарушений LAY7 нет"
