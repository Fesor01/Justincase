// Telegram notifications: messages only, never touches keys.
const TG_TOKEN = process.env.TELEGRAM_BOT_TOKEN;
const OWNER_CHAT = process.env.OWNER_CHAT_ID;
const GUARDIAN_CHATS = (process.env.GUARDIAN_CHAT_IDS || "")
  .split(",").map((x) => x.trim()).filter(Boolean);
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

async function tg(chatId, text) {
  if (TG_TOKEN && chatId) {
    try {
      const r = await fetch(`https://api.telegram.org/bot${TG_TOKEN}/sendMessage`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ chat_id: chatId, text }),
      });
      if (r.ok === false) log("telegram error:", r.status);
    } catch (e) {
      log("telegram failed:", e?.message ?? e);
    }
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
    await toGuardians("JustInCase: the vault has been released to the beneficiaries.");
  } else if (next === "Active") {
    await tg(OWNER_CHAT, "JustInCase: check-in received. Alert cancelled.");
    await toGuardians("JustInCase: the owner checked in. Alert cancelled, no action needed.");
  }
}
