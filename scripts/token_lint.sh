#!/usr/bin/env bash
# FR-046 (PRD-0006 F-7): токен-линт — hex-литералы цветов вне design-токенов.
#
# Правило: цветовой hex-литерал (0xRRGGBB или #rrggbb) в производственном
# .rs-коде рендера допустим ТОЛЬКО через design-токены
# (design/tokens/*.json ↔ canvas_core::tokens).
#
# Исключения (протокол ложных срабатываний, риск R5 PRD-0006):
# 1. canvas-shell/canvas-widgets/canvas-preview-host — Win32-домен: hex —
#    коды сообщений/стилей, не цвета;
# 2. тесты: строки после первого `#[cfg(test)]` и каталоги tests/;
# 3. комментарии (///, //) — документация, не код;
# 4. canvas-core/src/templates.rs — hex в контракте ДАННЫХ манифестов
#    шаблонов (canvasdesk.template.color, FR-018): это сериализуемые
#    значения .canvas/манифестов, а не рендер-палитра.
#
# Запуск: bash scripts/token_lint.sh
set -euo pipefail
cd "$(dirname "$0")/.."

ALLOWED='^(design/|docs/|sdk/|scripts/|assets/|crates/canvas-core/src/tokens\.rs|crates/canvas-core/src/templates\.rs|crates/canvas-shell/|crates/canvas-widgets/|crates/canvas-preview-host/|crates/.*/tests/|crates/.*/src/tests\.rs)'

hits=""
while IFS= read -r f; do
  # awk: отрезать всё после первого #[cfg(test)] (тесты — в конце файла)
  hits+=$(awk -v F="$f" '/#\[cfg\(test\)\]/{exit} {print F ":" $0}' "$f" \
    | grep -Ev '^[^:]+:[[:space:]]*(///|//!|//)' \
    | rg "0x[0-9A-Fa-f]{6}\b|#[0-9A-Fa-f]{6}\b" || true)
done < <(git ls-files '*.rs' | grep -Ev "$ALLOWED")

if [ -n "$hits" ]; then
  echo "token_lint: НАЙДЕНЫ цветовые hex-литералы вне design-токенов (G1 нарушен):"
  echo "$hits"
  echo
  echo "Добавьте значение в design/tokens/colors.json + canvas_core::tokens"
  echo "и используйте слот/алиас (см. PRD-0006 §7.1, FR-046 Р-4)."
  exit 1
fi

echo "token_lint: OK — цветовых hex-литералов вне токенов/исключений нет (G1)"
