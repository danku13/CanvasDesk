#!/usr/bin/env bash
# Окружение сборки CanvasDesk: тулчейн 1.99.0 + wasm32-unknown-unknown.
# Источник: source scripts/cargo-env.sh (из корня репо или откуда угодно).
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
    echo "cargo-env: rustup не найден (~/.cargo/bin отсутствует)" >&2
    return 0 2>/dev/null || exit 0
fi
rustup default 1.99.0 >/dev/null 2>&1 || true
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
# Диск контейнера мал (rootfs 10G) — тяжёлый target на PolarFS-томе.
export CARGO_TARGET_DIR=/tmp/my-project/cd-target
mkdir -p "$CARGO_TARGET_DIR"
