export P=jic-fesor01
source /workspaces/Justincase/scripts/helpers.sh
bal() { curl -s https://test.rpc.fastnear.com -H 'Content-Type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"method\":\"query\",\"params\":{\"request_type\":\"view_account\",\"finality\":\"final\",\"account_id\":\"$1\"}}" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const j=JSON.parse(s);console.log(process.argv[1],(Number(BigInt(j.result.amount)/10n**22n)/100).toFixed(2),"NEAR")})' "$1"; }
prep() { jic_call release check_in owner; jic_status release; }
before() { echo "--- BEFORE ---"; bal $P-b1.testnet; bal $P-b2.testnet; }
agent() { (cd /workspaces/Justincase/agent && VAULT_ACCOUNT=$P-release.testnet npm start); }
confirm() { jic_call release confirm g1; jic_call release confirm g2; jic_status release; }
after() { echo "--- AFTER ---"; bal $P-b1.testnet; bal $P-b2.testnet; jic_status release; }
