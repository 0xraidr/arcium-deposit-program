#!/usr/bin/env bash
# Copy build/deposit.arcis to test-fixtures for public HTTPS hosting (GitHub raw).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/build/deposit.arcis"
DST="$ROOT/test-fixtures/deposit.arcis"

if [[ ! -f "$SRC" ]]; then
  echo "Missing $SRC — run: cd arcium && arcium build" >&2
  exit 1
fi

cp "$SRC" "$DST"
echo "[sync] $DST ($(wc -c < "$DST") bytes)"
echo "[sync] Push to GitHub, then set in test/.env:"
echo "  DEPOSIT_CIRCUIT_URL=https://raw.githubusercontent.com/0xraidr/shard-program/main/arcium/test-fixtures/deposit.arcis"
