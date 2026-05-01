#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/apps/rfp-desktop"
TAURI_DIR="$APP_DIR/src-tauri"

echo "== RFP v2 verification =="

if [ ! -d "$APP_DIR" ]; then
  echo "apps/rfp-desktop does not exist yet."
  echo "Run Priority 1 Task 1 before full verification."
  exit 0
fi

if [ -f "$TAURI_DIR/Cargo.toml" ]; then
  echo "== Rust tests =="
  cargo test --manifest-path "$TAURI_DIR/Cargo.toml"
else
  echo "Skipping Rust tests: $TAURI_DIR/Cargo.toml not found."
fi

if [ -f "$APP_DIR/package.json" ]; then
  if [ -d "$APP_DIR/node_modules" ]; then
    echo "== Frontend tests =="
    if npm pkg get scripts.test --prefix "$APP_DIR" >/dev/null 2>&1; then
      npm run test --prefix "$APP_DIR"
    else
      echo "Skipping frontend tests: package.json has no test script."
    fi

    echo "== Frontend build =="
    npm run build --prefix "$APP_DIR"
  else
    echo "Skipping frontend tests/build: node_modules is not installed."
    echo "Run: npm install --prefix apps/rfp-desktop"
  fi
else
  echo "Skipping frontend checks: $APP_DIR/package.json not found."
fi

if [ -f "$TAURI_DIR/src/bin/smoke_first_pdf.rs" ]; then
  echo "== Smoke binary build =="
  cargo build --manifest-path "$TAURI_DIR/Cargo.toml" --bin smoke_first_pdf
else
  echo "Skipping smoke binary build: smoke_first_pdf.rs not found yet."
fi

echo "== Verification script finished =="

