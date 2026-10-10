#!/usr/bin/env bash
# LAY-W14 (ревью layouts-w1-w12-review.md: §3.1 P1-2, §3.2 P2-5/P2-6, §4):
# регрессионный гейт LAY7 — тонкая обёртка над python3-сканером
# scripts/lay7_scan.py (прецедент стиля — scripts/wasm_time_audit.py).
#
# Архитектура после ремонта гейта:
#   * сканер обходит ВСЕ *.rs под crates/canvas-app/src рекурсивно и сканирует
#     каждый файл ЦЕЛИКОМ; тестовый код отсекается по mod-границам, а не по
#     «первому #[cfg(test)]» — прежняя awk-отсечка обрубала app.rs на :156 и
#     lib.rs на :122 (~35 тыс. строк вне гейта, P1-2);
#   * паттерны (P2-6): gap-литералы — целые/дробные/отрицательные/скобочные
#     (исключение на уровне совпадения: 0/0.0 — нейтральный ритм раздела
#     «Исключения» 11-layouts.md; collision_gap/row_gap — не LAY7) и курсоры
#     x/y += с дробным литералом, включая смешанные `y += CONST + 6.0`;
#   * исключения — только инлайн: `// lay7:allow <причина>` в конце строки
#     подавляет ТОЛЬКО эту строку (файловый allowlist прежнего гейта удалён —
#     P2-5 «перманентная лазейка»);
#   * negative-тест самого гейта: `--selftest` — фикстуры вне репо, ожидания
#     rc=0/rc=1; ослабление сканера в PR такой PR не позеленит.
#
# Что НЕ гейтится (осознанно, полные границы — в шапке lay7_scan.py):
#   * чисто-переменные курсоры (`cursor_y += step;`, `y += line_h;`) — шаг из
#     именованной константы не создаёт нового off-scale литерала;
#   * всё вне crates/canvas-app/src: например canvas-render/text.rs:1390
#     `gap: 2.0` (content-layout) остаётся вне скана до решения LAY-W21.
#
# Запуск:  bash scripts/lay7_lint.sh            — скан дерева (rc 0/1)
#          bash scripts/lay7_lint.sh --selftest — negative-тест гейта (rc 0/1)
#          bash scripts/lay7_lint.sh <пути...>  — скан конкретных файлов/каталогов
# Выход:   0 — нарушений нет; 1 — есть (список file:line: fragment).
set -euo pipefail
cd "$(dirname "$0")/.."

SCAN=scripts/lay7_scan.py

selftest() {
  # Фикстуры генерируются во временном каталоге ВНЕ репо и проверяются
  # построчно: ожидаемый rc + (для FAIL) присутствие имени фикстуры в выводе
  # (отличаем детекцию от падения сканера).
  # tmp — глобальная переменная: EXIT-трап срабатывает после выхода из
  # функции, когда local-переменные уже не видны (set -u ловит это).
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/lay7-selftest.XXXXXX")"
  trap "rm -rf '$tmp'" EXIT

  cat > "$tmp/t01_decl_then_violation.rs" <<'EOF'
// Регресс P1-2: декларация тест-модуля в шапке НЕ должна глушить остаток файла.
#[cfg(test)]
mod ui_layout_lint;
fn body() -> Row {
    Row { gap: 9.0 }
}
EOF
  cat > "$tmp/t02_inline_mod_skipped.rs" <<'EOF'
// Инлайн тест-мод с нарушением внутри — пропускается целиком.
#[cfg(test)]
mod t {
    fn g() -> Row {
        Row { gap: 9.0 }
    }
}
EOF
  cat > "$tmp/t03_violation_after_inline_mod.rs" <<'EOF'
// После закрытия инлайн тест-мода скан продолжается.
#[cfg(test)]
mod t {
    fn g() {}
}
fn body() -> Row {
    Row { gap: 8 }
}
EOF
  cat > "$tmp/t04_row_gap_same_line.rs" <<'EOF'
// Построчный баг прежнего фильтра: hit по второму gap в той же строке.
fn body() -> Row {
    Row { row_gap: 2.0, gap: 9.0 }
}
EOF
  cat > "$tmp/t05_int_gap.rs" <<'EOF'
fn body() -> Row {
    Row { gap: 8 }
}
EOF
  cat > "$tmp/t06_neg_gap.rs" <<'EOF'
fn body() -> Row {
    Row { gap: -1.0 }
}
EOF
  cat > "$tmp/t07_paren_gap.rs" <<'EOF'
fn body() -> Row {
    Row { gap:(8.0) }
}
EOF
  cat > "$tmp/t08_mixed_cursor.rs" <<'EOF'
// Канонический пример аудита: смешанный курсор с дробным литералом.
fn body() {
    y += CONST + 6.0;
}
EOF
  cat > "$tmp/t09_x_cursor.rs" <<'EOF'
fn body() {
    x += 4.0;
}
EOF
  cat > "$tmp/t10_allow_markers.rs" <<'EOF'
// Инлайн-исключения подавляют только свою строку.
fn body() {
    y += CONST + 6.0; // lay7:allow off-scale 6.0 — кандидат W16
    x += 4.0; // lay7:allow декоративный оффсет, вне LAY7
    let _ = Row { gap: 9.0 }; // lay7:allow off-scale 9.0 — кандидат W16
    let _ = Row { gap: -1.0 }; // lay7:allow зазор анимации, вне LAY7
}
EOF
  cat > "$tmp/t11_neutral_values.rs" <<'EOF'
// 0.0 — нейтральный ритм («Исключения» 11-layouts.md); collision_gap — не LAY7.
fn body() {
    let _ = Row { gap: 0.0 };
    let _ = Snap { collision_gap: 8.0 };
}
EOF
  mkdir -p "$tmp/t12_ext_mod"
  cat > "$tmp/t12_ext_mod/lib.rs" <<'EOF'
// Внешний тест-модуль: файл skipme.rs пропускается целиком.
#[cfg(test)]
mod skipme;
fn body() -> Row {
    Row { gap: 0.0 }
}
EOF
  cat > "$tmp/t12_ext_mod/skipme.rs" <<'EOF'
// Этот файл — тест-модуль: нарушение внутри не должно всплывать.
#[test]
fn hidden() {
    let _ = Row { gap: 9.0 };
}
EOF

  local failures=0
  expect() { # $1 — путь фикстуры, $2 — PASS|FAIL, $3 — описание
    local out rc name
    name="$(basename "$1")"
    out="$(python3 "$SCAN" "$1" 2>&1)" && rc=0 || rc=$?
    local ok=0
    if [ "$2" = FAIL ]; then
      if [ "$rc" -eq 1 ] && grep -qF "$name" <<<"$out"; then ok=1; fi
    else
      if [ "$rc" -eq 0 ]; then ok=1; fi
    fi
    if [ "$ok" -eq 1 ]; then
      echo "  ok   $name — $3"
    else
      echo "  FAIL $name — $3 (ожидалось $2, rc=$rc)"
      echo "  --- вывод сканера ---"
      sed 's/^/  | /' <<<"$out"
      echo "  ---------------------"
      failures=$((failures + 1))
    fi
  }

  echo "LAY7 selftest: фикстуры в $tmp"
  expect "$tmp/t01_decl_then_violation.rs" FAIL "нарушение ПОСЛЕ декларации тест-мода ловится (P1-2)"
  expect "$tmp/t02_inline_mod_skipped.rs"  PASS "инлайн тест-мод с нарушением внутри пропущен"
  expect "$tmp/t03_violation_after_inline_mod.rs" FAIL "нарушение после закрытия инлайн тест-мода ловится"
  expect "$tmp/t04_row_gap_same_line.rs"   FAIL "row_gap: 2.0, gap: 9.0 — hit по второму gap"
  expect "$tmp/t05_int_gap.rs"             FAIL "gap: 8 — целый литерал ловится"
  expect "$tmp/t06_neg_gap.rs"             FAIL "gap: -1.0 — отрицательный литерал ловится"
  expect "$tmp/t07_paren_gap.rs"           FAIL "gap:(8.0) — скобочный литерал ловится"
  expect "$tmp/t08_mixed_cursor.rs"        FAIL "y += CONST + 6.0 — смешанный курсор ловится"
  expect "$tmp/t09_x_cursor.rs"            FAIL "x += 4.0 — горизонтальный курсор ловится"
  expect "$tmp/t10_allow_markers.rs"       PASS "// lay7:allow <причина> подавляет свою строку"
  expect "$tmp/t11_neutral_values.rs"      PASS "gap: 0.0 и collision_gap: 8.0 — исключения"
  expect "$tmp/t12_ext_mod"                PASS "файл внешнего тест-мода (mod x;) пропущен целиком"

  if [ "$failures" -ne 0 ]; then
    echo "LAY7 selftest: расхождений: $failures"
    return 1
  fi
  return 0
}

if [ "${1:-}" = "--selftest" ]; then
  if selftest; then
    echo
    echo "LAY7 selftest: OK — все фикстуры сошлись с ожиданиями"
    exit 0
  else
    echo
    echo "LAY7 selftest: FAIL — расхождения выше"
    exit 1
  fi
fi

rc=0
python3 "$SCAN" "$@" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo
  echo "Примечание: хиты в admin_ui.rs ожидаемы до волны LAY-W13"
  echo "(параллельная миграция тел админки на Column-скелет, ревью §3.1 P1-1)."
  echo "Для прочих строк — инлайн-исключение // lay7:allow <причина> или миграция."
fi
exit "$rc"
