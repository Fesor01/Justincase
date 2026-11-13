// Builds the handover letter. Uses NEAR AI Cloud if a key is set; otherwise a template.
// Reads notes only. Never passwords or seed phrases.
import fs from "node:fs";

const BASE = process.env.NEAR_AI_URL || "https://cloud-api.near.ai/v1";
const KEY = process.env.NEAR_AI_API_KEY;
const OWNER = process.env.OWNER_NAME || "the owner";
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

function loadNotes() {
  return JSON.parse(fs.readFileSync(new URL("../asset_map.json", import.meta.url), "utf8"));
}

function templateLetter(notes) {
  const lines = [
    "To my family,",
    "",
    `This letter was prepared by JustInCase. ${OWNER} stopped checking in, and the guardians they trusted confirmed it. The funds in the vault have been released to you. Here is where everything else is, in their own words:`,
    "",
  ];
  for (const a of notes.accounts || []) {
    lines.push(`- ${a.name}: ${a.where}. ${a.contact}.`);
  }
  if (notes.personal_message) lines.push("", notes.personal_message);
  return lines.join("\n");
}

async function pickModel() {
  if (process.env.NEAR_AI_MODEL) return process.env.NEAR_AI_MODEL;
  const r = await fetch(`${BASE}/models`, {
    headers: { Authorization: `Bearer ${KEY}` },
    signal: AbortSignal.timeout(15000),
  });
  const j = await r.json();
  const ids = (j.data || []).map((m) => m.id);
  return ids.find((i) => !/embed|image|whisper|rerank|tts/i.test(i)) || ids[0];
}

async function aiLetter(notes) {
  const model = await pickModel();
  if (!model) throw new Error("no model available");
  const r = await fetch(`${BASE}/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${KEY}` },
    body: JSON.stringify({
      model,
      temperature: 0.3,
      max_tokens: 700,
      messages: [
        {
          role: "system",
          content:
            "You write a warm, plain-language handover letter to a grieving family. Use ONLY the notes provided. Never invent accounts, numbers, names or instructions. Never ask for passwords. Keep it under 250 words.",
        },
        { role: "user", content: `Owner: ${OWNER}\nNotes:\n${JSON.stringify(notes, null, 2)}` },
      ],
    }),
    signal: AbortSignal.timeout(30000),
  });
  if (r.ok === false) throw new Error(`AI status ${r.status}`);
  const j = await r.json();
  const text = j.choices?.[0]?.message?.content?.trim();
  if (!text) throw new Error("empty AI reply");
  return text;
}

export async function buildLetter() {
  const notes = loadNotes();
  if (KEY) {
    try {
      const t = await aiLetter(notes);
      log("handover letter written by AI");
      return t;
    } catch (e) {
      log("AI letter failed, using template:", e?.message ?? e);
    }
  } else {
    log("no NEAR_AI_API_KEY, using template letter");
  }
  return templateLetter(notes);
}
