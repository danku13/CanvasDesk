#!/usr/bin/env bash
# Issue #68 волна V3 (гейты канона вертикали IN11–IN13):
# тонкая обёртка над python3-сканером scripts/v3_ui_scan.py
# (прецедент стиля — scripts/lay7_lint.sh, token_lint.sh).
#
# Три правила сканера:
#   V3-L1 «полный-rect label» — label_center/label_left, область которых не
#          выведена из kit::text_area_vcentered (канон: строковый бокс
#          size×1.3, центрированный в rect контрола; спека design/rules/
#          09-input.md IN11/IN13);
#   V3-L2 «магические вертикальные офсеты» на текст/каретка-сайтах —
#          литеральные y-сдвиги (+2…+12), ручные v-центрирующие формулы
#          ((h−14)/2, h/2−7, (h−16)*0.5), литеральный dy в push_line;
#   V3-L3 «литералы фактора строки» — ×1.3/×1.1 вне canvas-ui/measure.rs
#          (единственный владелец SCREEN_LINE_FACTOR).
#
# Исключения — только инлайн: `// v3:allow <причина>` в конце строки
# (для L1 — в любой строке скобочного спана вызова). Файловых allowlist нет.
# measure.rs исключён целиком (владелец канона — эталон, не нарушение).
#
# Запуск:  bash scripts/v3_ui_lint.sh            — скан дерева (rc 0/1)
#          bash scripts/v3_ui_lint.sh --selftest — negative-тест гейта (rc 0/1)
#          bash scripts/v3_ui_lint.sh <пути...>  — скан конкретных файлов/каталогов
# Выход:   0 — нарушений нет; 1 — есть (список file:line: fragment).
set -euo pipefail
cd "$(dirname "$0")/.."

SCAN=scripts/v3_ui_scan.py

selftest() {
  # Фикстуры генерируются во временном каталоге ВНЕ репо и проверяются
  # построчно: ожидаемый rc + (для FAIL) присутствие имени фикстуры в выводе.
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/v3-selftest.XXXXXX")"
  trap "rm -rf '$tmp'" EXIT

  # --- V3-L1: label-вызовы вне text_area_vcentered -------------------------
  cat > "$tmp/f01_l1_bare_call.rs" <<'EOF'
fn body(d: &mut KitDraw, r: UiRect) {
    d.label_center(r, "Готово", C, 13.0);
}
EOF
  cat > "$tmp/f02_l1_let_binding.rs" <<'EOF'
fn body(d: &mut KitDraw, r: UiRect) {
    let ta = canvas_ui::kit::text_area_vcentered(r, 13.0);
    d.label_center(ta, "Готово", C, 13.0);
}
EOF
  cat > "$tmp/f03_l1_inline_vcentered.rs" <<'EOF'
fn body(d: &mut KitDraw, r: UiRect) {
    d.label_center(canvas_ui::kit::text_area_vcentered(r, 13.0), "Готово", C, 13.0);
}
EOF
  cat > "$tmp/f04_l1_allow_marker_in_span.rs" <<'EOF'
fn body(d: &mut KitDraw, rect: UiRect) {
    d.label_left(
        rect, // v3:allow компонентный строковый бокс (IN11)
        "Подпись", C, 11.0,
    );
}
EOF
  cat > "$tmp/f04b_l1_standalone_marker_after_span.rs" <<'EOF'
fn body(d: &mut KitDraw, rect: UiRect) {
    d.label_left(
        rect,
        "Подпись", C, 11.0,
    );
    // v3:allow компонентный строковый бокс (IN13)
}
EOF
  cat > "$tmp/f05_l1_decl_not_call.rs" <<'EOF'
impl KitDraw {
    /// Декларация без точки-приёмника — не вызов.
    pub fn label_center(&mut self, area: UiRect, text: &str, color: [f32; 4], size: f32) {
        self.painter.label(area, text, color, size, PaintAlign::Center);
    }
}
EOF

  # --- V3-L2: магические вертикальные офсеты -------------------------------
  cat > "$tmp/f06_l2_origin_y_offset.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [r[0] + 8.0, r[1] + 5.0],
        text: line,
    };
}
EOF
  cat > "$tmp/f07_l2_variable_offset.rs" <<'EOF'
fn body() {
    let dy = line_h;
    let _ = OwnedScreenText {
        origin: [r[0] + 8.0, r[1] + dy],
        text: line,
    };
}
EOF
  cat > "$tmp/f08_l2_x_only_offset.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [r[0] + 8.0, r[1]],
        text: line,
    };
}
EOF
  cat > "$tmp/f09_l2_out_of_range.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [x, y + 40.0],
        text: line,
    };
}
EOF
  cat > "$tmp/f10_l2_token_offset.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [x, y + SPACING_SM],
        text: line,
    };
}
EOF
  cat > "$tmp/f11_l2_push_line_dy.rs" <<'EOF'
fn body() {
    let t = texts.push(push_line(line1, 4.0));
}
EOF
  cat > "$tmp/f12_l2_vcenter_formula.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [card[0], card[1] + card[3] / 2.0 - 8.5],
        text: line,
    };
}
EOF
  cat > "$tmp/f13_l2_multiline_origin.rs" <<'EOF'
fn body() {
    texts.push(OwnedScreenText {
        origin: [
            rect[0] + 8.0,
            rect[1] + 6.0,
        ],
        text: line,
    });
}
EOF
  cat > "$tmp/f14_l2_allow_marker_own_line.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [r[0], r[1] + 5.0], // v3:allow стек результатов (IN11)
        text: line,
    };
}
EOF
  cat > "$tmp/f15_l2_caret_manual.rs" <<'EOF'
fn body() {
    let caret = UiRect::new(x, area.y + 2.0, 1.0, area.h - 4.0);
}
EOF
  cat > "$tmp/f16_l2_caret_canonical.rs" <<'EOF'
fn body() {
    let caret = canvas_ui::kit::caret_rect(ta, 13.0, caret_x);
}
EOF
  cat > "$tmp/f17_l2_index_masking.rs" <<'EOF'
fn body() {
    let _ = OwnedScreenText {
        origin: [x, r[1] + win[3]],
        text: line,
    };
}
EOF
  cat > "$tmp/f18_l2_non_text_context.rs" <<'EOF'
fn body() {
    let pad = panel.h / 2.0 - 40.0;
}
EOF
  cat > "$tmp/f19_l2_comment_not_hit.rs" <<'EOF'
fn body() {
    // origin: [r[0], r[1] + 5.0] — документация, не код
}
EOF

  # --- отсечка тест-модов ----------------------------------------------------
  cat > "$tmp/t01_inline_mod_skipped.rs" <<'EOF'
#[cfg(test)]
mod t {
    fn hidden(d: &mut KitDraw, r: UiRect) {
        let _ = d.label_center(r, "x", C, 13.0);
    }
}
fn body() {}
EOF
  cat > "$tmp/t02_violation_after_inline_mod.rs" <<'EOF'
#[cfg(test)]
mod t {
    fn hidden() {}
}
fn body(d: &mut KitDraw, r: UiRect) {
    d.label_center(r, "x", C, 13.0);
}
EOF
  mkdir -p "$tmp/t03_ext_mod"
  cat > "$tmp/t03_ext_mod/lib.rs" <<'EOF'
#[cfg(test)]
mod skipme;
fn body() {}
EOF
  cat > "$tmp/t03_ext_mod/skipme.rs" <<'EOF'
fn hidden(d: &mut KitDraw, r: UiRect) {
    let _ = d.label_center(r, "x", C, 13.0);
}
EOF

  # --- V3-L3: литералы фактора строки ---------------------------------------
  cat > "$tmp/f20_l3_factor_literal.rs" <<'EOF'
fn body(size: f32) {
    let line_h = size * 1.3;
}
EOF
  cat > "$tmp/f21_l3_other_meaning.rs" <<'EOF'
fn body(fill: [f32; 4]) {
    let hover = fill * 1.1;
}
EOF
  mkdir -p "$tmp/f22_l3_measure_excluded"
  cat > "$tmp/f22_l3_measure_excluded/measure.rs" <<'EOF'
pub const SCREEN_LINE_FACTOR: f32 = 1.3;
pub fn line_height(size: f32) -> f32 {
    size * 1.3
}
EOF
  cat > "$tmp/f22_l3_measure_excluded/ok.rs" <<'EOF'
fn body() {
    let x = 1.0;
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

  echo "V3 selftest: фикстуры в $tmp"
  expect "$tmp/f01_l1_bare_call.rs"           FAIL "L1: label_center с голым rect ловится"
  expect "$tmp/f02_l1_let_binding.rs"         PASS "L1: let ta = text_area_vcentered — канон"
  expect "$tmp/f03_l1_inline_vcentered.rs"    PASS "L1: vcentered прямо в вызове — канон"
  expect "$tmp/f04_l1_allow_marker_in_span.rs" PASS "L1: allow-маркер в спане вызова гасит вызов"
  expect "$tmp/f04b_l1_standalone_marker_after_span.rs" PASS "L1: standalone-маркер после ');' гасит вызов (rustfmt)"
  expect "$tmp/f05_l1_decl_not_call.rs"       PASS "L1: декларация без приёмника не вызов"
  expect "$tmp/f06_l2_origin_y_offset.rs"     FAIL "L2: литеральный y-офсет в origin ловится"
  expect "$tmp/f07_l2_variable_offset.rs"     PASS "L2: переменное смещение не литерал"
  expect "$tmp/f08_l2_x_only_offset.rs"       PASS "L2: только x-офсет — не вертикаль"
  expect "$tmp/f09_l2_out_of_range.rs"        PASS "L2: офсет вне диапазона 0.5–12.5 — шаг стека"
  expect "$tmp/f10_l2_token_offset.rs"        PASS "L2: токен-офсет — не литерал"
  expect "$tmp/f11_l2_push_line_dy.rs"        FAIL "L2: литеральный dy в push_line ловится"
  expect "$tmp/f12_l2_vcenter_formula.rs"     FAIL "L2: ручная v-центрирующая формула ловится"
  expect "$tmp/f13_l2_multiline_origin.rs"    FAIL "L2: многострочный origin ловится целиком"
  expect "$tmp/f14_l2_allow_marker_own_line.rs" PASS "L2: allow-маркер гасит свою строку"
  expect "$tmp/f15_l2_caret_manual.rs"        FAIL "L2: ручная геометрия каретки ловится"
  expect "$tmp/f16_l2_caret_canonical.rs"     PASS "L2: caret_rect — канон"
  expect "$tmp/f17_l2_index_masking.rs"       PASS "L2: целочисленный индекс win[3] — не литерал"
  expect "$tmp/f18_l2_non_text_context.rs"    PASS "L2: офсет вне текст-контекста — зона LAY"
  expect "$tmp/f19_l2_comment_not_hit.rs"     PASS "L2: комментарий не код"
  expect "$tmp/t01_inline_mod_skipped.rs"     PASS "инлайн тест-мод с нарушением внутри пропущен"
  expect "$tmp/t02_violation_after_inline_mod.rs" FAIL "нарушение ПОСЛЕ закрытия инлайн тест-мода ловится (P1-2)"
  expect "$tmp/t03_ext_mod"                   PASS "файл внешнего тест-мода (mod x;) пропущен целиком"
  expect "$tmp/f20_l3_factor_literal.rs"      FAIL "L3: литерал * 1.3 в line/font-контексте ловится"
  expect "$tmp/f21_l3_other_meaning.rs"       PASS "L3: 1.1 вне line/font-контекста — иной смысл"
  expect "$tmp/f22_l3_measure_excluded"       PASS "L3: measure.rs — владелец канона, исключён"

  if [ "$failures" -ne 0 ]; then
    echo "V3 selftest: расхождений: $failures"
    return 1
  fi
  return 0
}

if [ "${1:-}" = "--selftest" ]; then
  if selftest; then
    echo
    echo "V3 selftest: OK — все фикстуры сошлись с ожиданиями"
    exit 0
  else
    echo
    echo "V3 selftest: FAIL — расхождения выше"
    exit 1
  fi
fi

rc=0
python3 "$SCAN" "$@" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo
  echo "Канон вертикали: design/rules/09-input.md IN11–IN13 (issue #68)."
  echo "Контрол-подпись — kit::text_area_vcentered(rect, size), каретка — kit::caret_rect;"
  echo "осознанный топ-анкерный стек (IN11) — инлайн-исключение // v3:allow <причина>."
fi
exit "$rc"
