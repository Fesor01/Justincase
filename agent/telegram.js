// Telegram notifications: messages only, never touches keys.
import { buildLetter } from "./letter.js";

const TG_TOKEN = process.env.TELEGRAM_BOT_TOKEN;
const OWNER_CHAT = process.env.OWNER_CHAT_ID;
const GUARDIAN_CHATS = (process.env.GUARDIAN_CHAT_IDS || "")
  .split(",").map((x) => x.trim()).filter(Boolean);
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

async function tg(chatId, text) {
  if (!TG_TOKEN || !chatId) return;
  for (let attempt = 1; attempt <= 4; attempt++) {
    try {
      const r = await fetch(`https://api.telegram.org/bot${TG_TOKEN}/sendMessage`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ chat_id: chatId, text }),
        signal: AbortSignal.timeout(15000),
      });
      if (r.ok) return;
      log("telegram error:", r.status);
      if (r.status < 500 && r.status !== 429) return;
    } catch (e) {
      log(`telegram attempt ${attempt} failed:`, e?.cause?.code ?? e?.message ?? e);
    }
    await new Promise((res) => setTimeout(res, 1500 * attempt));
  }
}

export async function onStateChange(prev, next) {
  if (prev === null) return; // no messages on agent startup
  const toGuardians = (t) => Promise.all(GUARDIAN_CHATS.map((c) => tg(c, t)));
  if (next === "Alert") {
    await tg(OWNER_CHAT, "JustInCase: your check-in was missed and the vault is now in ALERT. Your guardians have been told. Check in now to cancel.");
    await toGuardians("JustInCase: the owner missed a check-in. If you can't reach them, you can confirm on the vault. If they turn out to be fine, no action is needed.");
  } else if (next === "PendingRelease") {
    await tg(OWNER_CHAT, "JustInCase: guardians confirmed. The cooldown has started. Check in now to cancel the release.");
    await toGuardians("JustInCase: enough guardians have confirmed. The cooldown has started. The owner can still cancel by checking in.");
  } else if (next === "Released") {
    await tg(OWNER_CHAT, "JustInCase: the vault has been released.");
    await toGuardians("JustInCase: the vault has been released to the beneficiaries. Here is the handover letter to share with the family:");
    try {
      const letter = (await buildLetter()).slice(0, 3800);
      await toGuardians(letter);
    } catch (e) {
      log("letter failed:", e?.message ?? e);
    }
  } else if (next === "Active") {
    await tg(OWNER_CHAT, "JustInCase: check-in received. Alert cancelled.");
    await toGuardians("JustInCase: the owner checked in. Alert cancelled, no action needed.");
  }
}
