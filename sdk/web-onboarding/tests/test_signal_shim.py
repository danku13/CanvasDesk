#!/usr/bin/env python3
"""Test runner for the FR-028 v2 JS-side signal shim.

Loads `signal-shim-test.html` (which inlines the same Ctrl+P / dblclick
detection that ships in `crates/canvas-web/index.html`) and verifies
that:
  1. dblclick on canvas → emits `canvas:note-created` signal
  2. Ctrl+P keydown → emits `canvas:palette-opened` signal (after 120ms)

The shim is the production fallback for tour scenarios that use
passive+waitFor(signal) gates. Real Rust-side wiring
(AppEvent::TourSignal + canvas_web::tour_signal::emit) is documented
as a TODO; this shim makes paletteTour and firstRunInline work today.

Run via:
    python3 sdk/web-onboarding/tests/test_signal_shim.py
"""
from __future__ import annotations

import sys
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent
TEST_PAGE = ROOT / "signal-shim-test.html"
TEST_URL = "file://" + str(TEST_PAGE)


def main() -> int:
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        errors: list[str] = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        page.goto(TEST_URL)
        page.wait_for_timeout(3000)
        log_text = page.evaluate(
            'document.getElementById("log")?.textContent || ""'
        )
        browser.close()

    print("Log output:")
    for line in log_text.split("["):
        if line.strip():
            print(f"  [{line}")

    if errors:
        print(f"\nFAIL: page errors: {errors}")
        return 1

    if "ALL TESTS PASSED" not in log_text:
        print("\nFAIL: signal-shim tests did not report ALL TESTS PASSED")
        return 1

    print("\nOK: signal-shim tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
