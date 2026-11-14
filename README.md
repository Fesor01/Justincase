# JustInCase

**An AI agent checks on you. If you go silent, your trusted guardians confirm, and a NEAR contract releases your funds to your family. The AI can never move the money.**

Built for NEAR HACKATHON. Tracks: AI Security & Defense, Open Innovation.

Demo post on X: _(link added after posting)_

## The problem

When someone dies or loses access, their family often can't find or reach their money and accounts. In crypto, lost keys mean the funds are gone for good. In Web2, it means months of paperwork and accounts nobody knew existed. Almost nobody sets anything up, because the topic is uncomfortable.

## What it does

1. The owner deposits funds into a vault contract and names beneficiaries (with percentage shares) and guardians.
2. The owner checks in on a schedule. A missed check-in lets the agent (or any guardian) move the vault to ALERT.
3. Guardians confirm. At the threshold (2 of 3 in the demo), a cooldown starts.
4. The owner can cancel at any point by checking in.
5. If the cooldown ends with no check-in, the vault releases and splits the funds between beneficiaries.
6. On release, the agent sends a plain-language handover letter, written from the owner's notes, telling the family where everything else is (bank, insurance, email).

States: `Active -> Alert -> PendingRelease -> Released`. A check-in returns the vault to `Active` from any state except `Released`.

## How NEAR is used

- **NEAR smart contract (Rust, near-sdk-rs):** holds the funds and enforces every rule: who can check in, who can alert, how many guardians must confirm, how long the cooldown lasts, and how funds are split.
- **Separate agent account:** the agent has its own NEAR account and key. The contract only lets it raise an alert, and `release` is open to anyone but only works once the rules are met.
- **NEAR AI Cloud:** writes the handover letter from the owner's notes. If the AI is unavailable, a template letter is used instead.

## Security model

- The AI never holds a key that can move vault funds.
- The agent cannot check in, because that would let it fake that the owner is alive.
- The agent cannot confirm, change settings, or withdraw.
- Guardians can also raise the alert, so the system still works if the agent's server is down.
- Worst case if the agent is compromised: a false ALERT. The owner's next check-in cancels it.
- Failed payouts (for example, a beneficiary account that doesn't exist) are recorded as unpaid and can be retried, so they don't get stuck.
- The owner can change timers or withdraw only while the vault is Active. Once an alert is raised, neither is possible.
- Trust model: guardians are trusted. If a threshold of them confirm and the owner stays silent through the cooldown, the funds are released. The owner can cancel at any time before release with a check-in.

## Try it (do X, see Y)

You don't need any accounts or keys for steps 1 to 3.

1. **Run the contract tests.**
   ```
   cd contract && cargo test
   ```
   See: `test result: ok. 12 passed; 0 failed`.

2. **Read a vault that was released by the agent on testnet** (requires near-cli-rs).
   ```
   near contract call-function as-read-only jic-fesor01-rehearse.testnet get_status json-args '{}' network-config testnet now
   ```
   See: `"state":"Released"`.

3. **Same check without installing anything** (needs curl and Node).
   ```
   curl -s https://test.rpc.fastnear.com -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":"1","method":"query","params":{"request_type":"call_function","finality":"final","account_id":"jic-fesor01-rehearse.testnet","method_name":"get_status","args_base64":"e30="}}' | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const j=JSON.parse(s);console.log(Buffer.from(j.result.result).toString())})'
   ```
   See: the same state, `Released`.

4. **Inspect the transactions** for that vault on a testnet explorer: https://testnet.nearblocks.io/address/jic-fesor01-rehearse.testnet
   See: `raise_alert`, two `confirm` calls and `release`, plus the split to the two beneficiary accounts.

### Run the full flow on your own testnet accounts

This takes 20+ minutes because the contract needs nine testnet accounts. You need Rust, `cargo-near`, `near-cli-rs` and Node 20+.

1. Create testnet accounts with a common prefix `P`: `P-owner`, `P-agent`, `P-g1`, `P-g2`, `P-g3`, `P-b1`, `P-b2`, and a vault account such as `P-demo` (all `.testnet`).
2. Build the contract: `cd contract && cargo near build non-reproducible-wasm`
3. Deploy: `export P=<your prefix> && ./scripts/deploy-vault.sh demo 20 20` (20-second check-in interval and cooldown).
4. Copy `.env.example` to `.env` and fill in the vault, agent account and agent key. Add a Telegram bot token and chat IDs if you want notifications, and a NEAR AI Cloud key if you want the AI letter. Never commit `.env`.
5. Start the agent: `cd agent && npm install && npm start`
6. Wait 20 seconds. See: `state: Active -> Alert` and an alert message.
7. In another terminal: `source scripts/helpers.sh`, then `jic_call demo confirm g1` and `jic_call demo confirm g2`. See: `PendingRelease`.
8. Wait for the cooldown. See: the agent calls `release`, the state becomes `Released`, and the funds split 60/40 between the two beneficiaries.
9. To see the safety net, check in with `jic_call demo check_in owner` before the cooldown ends. See: the vault returns to `Active`.

## Repository layout

- `contract/` - the Rust vault contract and its 12 unit tests
- `agent/` - Node agent (`agent.js`), Telegram messages (`telegram.js`), handover letter (`letter.js`)
- `scripts/` - `deploy-vault.sh` and `helpers.sh`
- `asset_map.json` - demo notes for the handover letter (invented data; it never holds passwords or seed phrases)

## Built during the hackathon

All code in this repository was written between November 9 and 14, 2026 by one person, using Claude as an AI coding assistant. Most of the code is AI-generated and was tested by hand on testnet.

Libraries and tools: near-sdk-rs, near-api-js 7.4.0, near-cli-rs, cargo-near, the Telegram Bot API, and NEAR AI Cloud (OpenAI-compatible API).

## Limitations

- A testnet prototype. Not audited, and not a legal will.
- Demo timers are 20 seconds. Real use would use days or weeks.
- No web interface yet. Check-ins and guardian confirmations are command-line calls.
- Telegram delivery can be delayed, and a retry may deliver a message twice.
- The handover letter is generated by an AI from the owner's notes and may phrase things differently on each run. Notes are sent to NEAR AI Cloud; no private-execution claim is made here.
- Native NEAR only. Stablecoins and other tokens are not supported.

## Next

A web page for check-ins and guardian confirmations, longer real-world timers, support for stablecoins, and a safe way to hand over sensitive recovery instructions.

## License

MIT