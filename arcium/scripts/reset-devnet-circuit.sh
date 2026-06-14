#!/usr/bin/env bash
# Close stale devnet comp def + buffers so tests re-upload fresh deposit.arcis.
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

PROGRAM="D29HHVZiZWc1cbR8nDnjfTg8CHEJmLrau3NbcnteFzi"
OFFSET="2029763011"
CLUSTER="456"
RPC="$ANCHOR_PROVIDER_URL"
KEY="$ANCHOR_WALLET"

echo "[reset] deactivate comp def offset=$OFFSET program=$PROGRAM"
if ! arcium deactivate-computation-definition \
  -o "$OFFSET" -p "$PROGRAM" -k "$KEY" --rpc-url "$RPC"; then
  echo "[reset] deactivate skipped (may already be deactivated)"
fi

echo "[reset] waiting 75s for deactivation TTL…"
sleep 75

echo "[reset] close raw circuit buffer index 0"
arcium close-computation-definition-buffers \
  -o "$OFFSET" -p "$PROGRAM" -i 0 -k "$KEY" --rpc-url "$RPC"

echo "[reset] close comp def on cluster $CLUSTER"
arcium close-computation-definition \
  -o "$OFFSET" -p "$PROGRAM" -c "$CLUSTER" -k "$KEY" --rpc-url "$RPC"

echo "[reset] done — run: cd arcium && yarn test:devnet"
echo "[reset] expect ~12 min circuit upload on first test run (Helius RPC recommended)"
