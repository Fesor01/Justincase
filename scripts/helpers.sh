# Source this file: source scripts/helpers.sh   (needs P set, e.g. export P=jic-fesor01)
: "${P:?Set P first, e.g. export P=jic-fesor01}"

# jic_status <vault>  ->  prints the state and whether a check-in is overdue
jic_status() {
  near contract call-function as-read-only "$P-$1.testnet" get_status json-args '{}' network-config testnet now 2>/dev/null \
    | grep -E '"(state|overdue)"' | tr -d ' \n'
  echo
}

# jic_call <vault> <method> <caller> [json-args]  ->  sends a transaction, prints the contract's logs and any error
jic_call() {
  local args="${4-}"
  [ -z "$args" ] && args='{}'
  local out
  out=$(near contract call-function as-transaction "$P-$1.testnet" "$2" json-args "$args" prepaid-gas '100 Tgas' attached-deposit '0 NEAR' sign-as "$P-$3.testnet" network-config testnet sign-with-legacy-keychain send 2>&1)
  echo "$out" | sed -n '/Function execution logs/,/Here is your console/p' | grep -v 'Here is your' | grep -v '^[[:space:]]*$'
}
