// JustInCase agent (v0, console only).
// Watches one vault. Can only call two rule-gated methods: raise_alert and release.
// It cannot check in, confirm, change settings, or withdraw.
import { Account, JsonRpcProvider, teraToGas } from "near-api-js";

const RPC = process.env.NEAR_RPC_URL || "https://test.rpc.fastnear.com";
const VAULT = process.env.VAULT_ACCOUNT;
const AGENT = process.env.AGENT_ACCOUNT;
const KEY = process.env.NEAR_AGENT_PRIVATE_KEY;
const POLL_MS = Number(process.env.POLL_SECONDS || 5) * 1000;

for (const [name, value] of Object.entries({
  VAULT_ACCOUNT: VAULT,
  AGENT_ACCOUNT: AGENT,
  NEAR_AGENT_PRIVATE_KEY: KEY,
})) {
  if (!value) {
    console.error(`Missing ${name}. See .env.example`);
    process.exit(1);
  }
}

const provider = new JsonRpcProvider({ url: RPC });
const account = new Account(AGENT, provider, KEY);
const log = (...args) => console.log(new Date().toISOString().slice(11, 19), ...args);

async function getStatus() {
  return provider.callFunction({ contractId: VAULT, method: "get_status", args: {} });
}

async function act(method) {
  log(`-> calling ${method} on ${VAULT}`);
  try {
    await account.callFunction({
      contractId: VAULT,
      methodName: method,
      args: {},
      gas: teraToGas("100"),
    });
    log(`   ${method} sent`);
  } catch (e) {
    log(`   ${method} failed: ${e?.message ?? e}`);
  }
}

let lastState = null;
let busy = false;

async function tick() {
  if (busy) return;
  busy = true;
  try {
    const s = await getStatus();
    if (s.state !== lastState) {
      log(`state: ${lastState ?? "?"} -> ${s.state}`);
      lastState = s.state;
    }
    if (s.state === "Active" && s.overdue) {
      await act("raise_alert");
    } else if (s.state === "PendingRelease" && s.now_ms >= s.release_at_ms) {
      await act("release");
    }
  } catch (e) {
    log("could not read the vault:", e?.message ?? e);
  } finally {
    busy = false;
  }
}

log(`agent ${AGENT} watching ${VAULT} every ${POLL_MS / 1000}s`);
(async function loop() {
  await tick();
  setTimeout(loop, POLL_MS);
})();
