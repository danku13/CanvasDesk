#!/usr/bin/env python3
"""Regression tests for the CanvasDesk inline-onboarding engine.

Runs against `sdk/web-onboarding/standalone-test.html` (no Rust/WASM
required — the test page mocks the #w6-toolbar anchors the scenarios
need). Launch via:

    python3 sdk/web-onboarding/tests/test_tour_regression.py

Tests cover:
  - scenario start (onStart callback fires, tooltip mounts)
  - step navigation (Next / Back / Done)
  - tooltip title + progress text
  - passive step + waitFor(signal) → auto-advance
  - dim overlay disabled on passive steps (clicks pass through)
  - URL hash #tour=<id> → auto-launch on load
  - tour:<id>:complete signal emitted on Done
  - Esc → skip (skippable scenarios)
  - Arrow keys → next/back
"""
from __future__ import annotations

import sys
from pathlib import Path

from playwright.sync_api import sync_playwright, expect, Page

ROOT = Path(__file__).resolve().parent.parent  # sdk/web-onboarding/
TEST_PAGE = ROOT / "standalone-test.html"
TEST_URL = "file://" + str(TEST_PAGE)


def wait_for_tooltip(page: Page, title: str | None = None, timeout: int = 4000):
    """Wait until a tooltip is mounted and (optionally) has the given title."""
    if title is None:
        expect(page.locator(".cd-tour-tooltip")).to_be_visible(timeout=timeout)
        return
    loc = page.locator(".cd-tour-tooltip__title", has_text=title)
    expect(loc).to_be_visible(timeout=timeout)


def click_primary(page: Page):
    page.eval_on_selector(".cd-tour-btn_primary", "el => el.click()")


def get_progress(page: Page) -> str:
    return page.eval_on_selector(
        ".cd-tour-tooltip__progress",
        "el => el.textContent",
    )


# ── tests ─────────────────────────────────────────────────────────

def test_toolbar_tour_full_flow(page: Page):
    """cd-toolbar-tour: 5 steps, click through, complete signal fires."""
    completed = []

    page.expose_binding("tour_complete_signal", lambda src, sig: completed.append(sig))
    page.goto(TEST_URL)

    # Subscribe to engine signals BEFORE running the tour.
    page.evaluate("""
        () => window.__canvasdeskTour.onSignal((name) => {
            if (name.indexOf("tour:") === 0) window.tour_complete_signal(name);
        })
    """)

    # Launch via engine.run() (avoid click-block by dim overlay).
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario
        )
    """)
    wait_for_tooltip(page, "Панель хранилища")
    assert get_progress(page) == "1 / 5"

    # Next through all 5 steps.
    expected_titles = [
        "Панель хранилища",
        "Открыть с диска",
        "Недавние",
        "Экспорт .canvas",
        "Готово",
    ]
    for i, title in enumerate(expected_titles):
        wait_for_tooltip(page, title)
        assert get_progress(page) == f"{i + 1} / 5", \
            f"step {i + 1}: progress wrong"
        click_primary(page)

    # After last click, tour should unmount.
    page.wait_for_timeout(500)
    # eval_on_selector throws on 0 matches — use querySelectorAll length
    # via page.evaluate instead, which returns 0 cleanly.
    count = page.evaluate(
        "() => document.querySelectorAll('.cd-tour-tooltip').length"
    )
    assert count == 0, f"tooltip should be unmounted, found {count}"

    # Complete signal should have fired.
    assert "tour:cd-toolbar-tour:complete" in completed, \
        f"complete signal missing; got {completed}"


def test_passive_step_auto_advances_on_signal(page: Page):
    """cd-first-run-inline step 3 is passive+waitFor(signal):
    no Next button; emit canvas:note-created → auto-advance."""
    page.goto(TEST_URL)

    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.firstRunInlineScenario
        )
    """)
    wait_for_tooltip(page, "Это интерактивный тур")

    # Advance to step 3 (passive).
    click_primary(page)
    click_primary(page)
    page.wait_for_timeout(400)
    wait_for_tooltip(page, "Создайте первую заметку")
    assert get_progress(page) == "3 / 8"

    # Verify primary is hidden (passive step).
    display = page.eval_on_selector(
        ".cd-tour-btn_primary",
        "el => getComputedStyle(el).display",
    )
    assert display == "none", f"primary should be hidden; got {display}"

    # Verify dim is disabled (opacity 0, pointer-events none).
    dim_opacity = page.eval_on_selector(
        ".cd-tour-dim", "el => getComputedStyle(el).opacity")
    dim_pe = page.eval_on_selector(
        ".cd-tour-dim", "el => getComputedStyle(el).pointerEvents")
    assert dim_opacity == "0", f"dim opacity should be 0; got {dim_opacity}"
    assert dim_pe == "none", f"dim pointer-events should be none; got {dim_pe}"

    # Emit the waited-for signal.
    page.evaluate("() => window.__canvasdeskTour.signal('canvas:note-created')")
    page.wait_for_timeout(500)

    # Tour auto-advanced.
    wait_for_tooltip(page, "Попробуйте формулу Numi")
    assert get_progress(page) == "4 / 8"


def test_palette_tour_passive_advance(page: Page):
    """cd-palette-tour step 2 is passive+waitFor(canvas:palette-opened)."""
    page.goto(TEST_URL)
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.paletteTourScenario
        )
    """)
    wait_for_tooltip(page, "Палитра шаблонов")
    click_primary(page)
    wait_for_tooltip(page, "Откройте палитру")
    assert get_progress(page) == "2 / 5"

    page.evaluate("() => window.__canvasdeskTour.signal('canvas:palette-opened')")
    page.wait_for_timeout(400)
    wait_for_tooltip(page, "Категории")
    assert get_progress(page) == "3 / 5"


def test_calculations_tour_navigation(page: Page):
    """cd-calculations-tour: 7 steps, all have Next button (non-passive)."""
    page.goto(TEST_URL)
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.calculationsTourScenario
        )
    """)
    wait_for_tooltip(page, "Формулы прямо в заметках")
    assert get_progress(page) == "1 / 7"

    # Walk through all 7 steps.
    for i in range(7):
        assert get_progress(page) == f"{i + 1} / 7"
        click_primary(page)
        page.wait_for_timeout(300)


def test_deeplink_url_hash_auto_launches(page: Page):
    """#tour=cd-toolbar-tour in URL → scenario runs on DOMContentLoaded."""
    url = TEST_URL + "#tour=cd-toolbar-tour"
    page.goto(url)
    wait_for_tooltip(page, "Панель хранилища", timeout=5000)
    assert get_progress(page) == "1 / 5"


def test_deeplink_unknown_id_warns(page: Page):
    """#tour=nonexistent → no tooltip, console warning."""
    url = TEST_URL + "#tour=nonexistent-scenario"
    msgs: list = []
    page.on("console", lambda m: msgs.append(m))
    page.goto(url)
    page.wait_for_timeout(800)
    count = page.evaluate(
        "() => document.querySelectorAll('.cd-tour-tooltip').length"
    )
    assert count == 0, f"no tooltip expected for unknown id; got {count}"
    found_warning = any(
        "scenario not found" in m.text for m in msgs
    )
    assert found_warning, "console.warn missing for unknown scenario id"


def test_esc_skip_closes_tour(page: Page):
    """Esc → skip (skippable=true). Tour unmounts."""
    page.goto(TEST_URL)
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario
        )
    """)
    wait_for_tooltip(page, "Панель хранилища")
    page.keyboard.press("Escape")
    page.wait_for_timeout(400)
    count = page.evaluate(
        "() => document.querySelectorAll('.cd-tour-tooltip').length"
    )
    assert count == 0, f"Esc should close the tour; found {count} tooltips"


def test_back_button_navigation(page: Page):
    """Back button — step N → N-1."""
    page.goto(TEST_URL)
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario
        )
    """)
    wait_for_tooltip(page, "Панель хранилища")
    click_primary(page)
    wait_for_tooltip(page, "Открыть с диска")
    assert get_progress(page) == "2 / 5"

    # Click Back.
    page.eval_on_selector(".cd-tour-btn_back", "el => el.click()")
    page.wait_for_timeout(300)
    assert get_progress(page) == "1 / 5", "Back didn't return to step 1"


def test_arrow_keys_navigation(page: Page):
    """ArrowRight → next; ArrowLeft → back."""
    page.goto(TEST_URL)
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario
        )
    """)
    wait_for_tooltip(page, "Панель хранилища")
    page.keyboard.press("ArrowRight")
    page.wait_for_timeout(300)
    assert get_progress(page) == "2 / 5", "ArrowRight didn't advance"
    page.keyboard.press("ArrowLeft")
    page.wait_for_timeout(300)
    assert get_progress(page) == "1 / 5", "ArrowLeft didn't go back"


def test_persistence_completion(page: Page):
    """Complete a scenario → isCompleted() returns true;
    resetCompleted() → false."""
    page.goto(TEST_URL)
    # Clear any prior state from previous test runs.
    page.evaluate("""
        () => window.__canvasdeskTour.resetCompleted('cd-toolbar-tour')
    """)
    assert page.evaluate("() => window.__canvasdeskTour.isCompleted('cd-toolbar-tour')") is False

    # Run + complete the toolbar tour.
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario
        )
    """)
    wait_for_tooltip(page, "Панель хранилища")
    for _ in range(5):
        click_primary(page)
        page.wait_for_timeout(200)
    page.wait_for_timeout(400)

    assert page.evaluate("() => window.__canvasdeskTour.isCompleted('cd-toolbar-tour')") is True
    ts = page.evaluate("() => window.__canvasdeskTour.getCompletedAt('cd-toolbar-tour')")
    assert ts is not None and ts > 0, f"completion timestamp missing; got {ts}"

    # skipIfCompleted should now refuse to re-run.
    result = page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario,
            { skipIfCompleted: true }
        )
    """)
    assert result is None, f"skipIfCompleted should refuse; got {result}"
    count = page.evaluate("() => document.querySelectorAll('.cd-tour-tooltip').length")
    assert count == 0, f"skipIfCompleted should not start a tour; found {count}"

    # resetCompleted re-enables running.
    page.evaluate("() => window.__canvasdeskTour.resetCompleted('cd-toolbar-tour')")
    assert page.evaluate("() => window.__canvasdeskTour.isCompleted('cd-toolbar-tour')") is False


def test_persistence_resume(page: Page):
    """Skip mid-tour → resume index persisted → run(resume) starts from there."""
    page.goto(TEST_URL)
    # Clean state.
    page.evaluate("() => window.__canvasdeskTour.resetCompleted('cd-toolbar-tour')")

    # Run + advance to step 2 + skip (Esc).
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario
        )
    """)
    wait_for_tooltip(page, "Панель хранилища")
    click_primary(page)
    wait_for_tooltip(page, "Открыть с диска")
    page.keyboard.press("Escape")
    page.wait_for_timeout(400)
    assert page.evaluate("() => document.querySelectorAll('.cd-tour-tooltip').length") == 0

    # Resume state persisted at step 1 (index 1, "Открыть с диска").
    assert page.evaluate("() => window.__canvasdeskTour.getResumeIndex('cd-toolbar-tour')") == 1

    # Run with resume=true → should start at step 2.
    page.evaluate("""
        () => window.__canvasdeskTour.run(
            window.CanvasDeskTour.scenarios.toolbarTourScenario,
            { resume: true }
        )
    """)
    page.wait_for_timeout(400)
    wait_for_tooltip(page, "Открыть с диска")
    assert get_progress(page) == "2 / 5", f"resume should start at step 2; got {get_progress(page)}"

    # Clean up: complete the tour.
    for _ in range(4):
        click_primary(page)
        page.wait_for_timeout(200)
    page.wait_for_timeout(400)
    # After completion, resume state should be cleared.
    assert page.evaluate("() => window.__canvasdeskTour.getResumeIndex('cd-toolbar-tour')") is None


# ── runner ────────────────────────────────────────────────────────

def main():
    tests = [
        ("toolbar_tour_full_flow", test_toolbar_tour_full_flow),
        ("passive_step_auto_advances_on_signal", test_passive_step_auto_advances_on_signal),
        ("palette_tour_passive_advance", test_palette_tour_passive_advance),
        ("calculations_tour_navigation", test_calculations_tour_navigation),
        ("deeplink_url_hash_auto_launches", test_deeplink_url_hash_auto_launches),
        ("deeplink_unknown_id_warns", test_deeplink_unknown_id_warns),
        ("esc_skip_closes_tour", test_esc_skip_closes_tour),
        ("back_button_navigation", test_back_button_navigation),
        ("arrow_keys_navigation", test_arrow_keys_navigation),
        ("persistence_completion", test_persistence_completion),
        ("persistence_resume", test_persistence_resume),
    ]
    print(f"\n=== Running {len(tests)} regression tests ===\n")
    passed = 0
    failed = 0
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        for name, fn in tests:
            page = browser.new_page()
            try:
                fn(page)
                print(f"  ✓ {name}")
                passed += 1
            except AssertionError as e:
                print(f"  ✗ {name}: {e}")
                failed += 1
            except Exception as e:
                print(f"  ✗ {name}: {type(e).__name__}: {e}")
                failed += 1
            finally:
                page.close()
        browser.close()
    print(f"\n=== {passed} passed, {failed} failed ===\n")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
