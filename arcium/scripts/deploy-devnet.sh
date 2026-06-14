#!/usr/bin/env bash
# Deploy encrypted_deposit to devnet (program-only upgrade — no circuit reset needed for callback fixes).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"
ENV_FILE="$REPO/test/.env"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source <(grep -v '^#' "$ENV_FILE" | grep -v '^$' | sed 's/^/export /')
  set +a
fi

export ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-${SOLANA_RPC_URL:-https://api.devnet.solana.com}}"
export ANCHOR_WALLET="${ANCHOR_WALLET:-${KEYPAIR_PATH:-$HOME/.config/solana/id.json}}"
export ANCHOR_WALLET="${ANCHOR_WALLET/#\~/$HOME}"

SKIP_BUILD=0
for arg in "$@"; do
  if [[ "$arg" == "--skip-build" ]]; then
    SKIP_BUILD=1
  fi
done

cd "$ROOT"

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  echo "[deploy] building encrypted_deposit…"
  CARGO_TARGET_DIR="$ROOT/target" anchor build
fi

SO="$ROOT/target/deploy/encrypted_deposit.so"
KEYPAIR="$ROOT/target/deploy/encrypted_deposit-keypair.json"
if [[ ! -f "$SO" ]]; then
  echo "Missing $SO — run anchor build first" >&2
  exit 1
fi

echo "[deploy] upgrading $SO on devnet ($ANCHOR_PROVIDER_URL)…"
solana program deploy "$SO" \
  --program-id "$KEYPAIR" \
  --url "$ANCHOR_PROVIDER_URL" \
  --max-sign-attempts 50 \
  --with-compute-unit-price 10000

echo "[deploy] done — run: cd arcium && yarn test:devnet"
