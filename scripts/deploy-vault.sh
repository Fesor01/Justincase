#!/usr/bin/env bash
# Deploy and initialise a JustInCase vault on testnet.
# Usage: P=<account prefix> ./scripts/deploy-vault.sh <vault-suffix> [checkin_secs] [cooldown_secs]
# Optional: B1=<account> B2=<account> to override the two beneficiaries (60% / 40%).
set -euo pipefail
: "${P:?Set P first, e.g. export P=jic-fesor01}"
VAULT="$P-$1.testnet"
INTERVAL="${2:-20}"
COOLDOWN="${3:-20}"
B1="${B1:-$P-b1.testnet}"
B2="${B2:-$P-b2.testnet}"
RPC=https://test.rpc.fastnear.com
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM="$ROOT/contract/target/near/contract.wasm"

[ -f "$WASM" ] || { echo "Build first: cd contract && cargo near build non-reproducible-wasm"; exit 1; }

exists() {
  curl -s "$RPC" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"method\":\"query\",\"params\":{\"request_type\":\"view_account\",\"finality\":\"final\",\"account_id\":\"$1\"}}" \
    | grep -q '"amount"'
}
for ACC in "$P-owner.testnet" "$P-agent.testnet" "$P-g1.testnet" "$P-g2.testnet" "$P-g3.testnet" "$B1" "$B2"; do
  exists "$ACC" || { echo "Account $ACC does not exist"; exit 1; }
done
for i in 1 2 3 4 5 6; do
  exists "$VAULT" && break
  echo "Waiting for $VAULT to appear on the network..."; sleep 5
done
exists "$VAULT" || { echo "Vault account $VAULT does not exist"; exit 1; }

ARGS=$(cat <<JSON
{"owner":"$P-owner.testnet","agent":"$P-agent.testnet","guardians":["$P-g1.testnet","$P-g2.testnet","$P-g3.testnet"],"threshold":2,"beneficiaries":[{"account_id":"$B1","bps":6000},{"account_id":"$B2","bps":4000}],"checkin_interval_secs":$INTERVAL,"cooldown_secs":$COOLDOWN}
JSON
)
echo "Deploying to $VAULT (check-in every ${INTERVAL}s, cooldown ${COOLDOWN}s)"
near contract deploy "$VAULT" use-file "$WASM" with-init-call new json-args "$ARGS" prepaid-gas '100 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-legacy-keychain send
