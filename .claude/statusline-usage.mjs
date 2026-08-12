#!/usr/bin/env node
// Per-model token/cost segment for the status line.
// Reads the statusline JSON on stdin, aggregates usage from the session's main
// transcript AND every subagent transcript (<session-dir>/subagents/*.jsonl),
// so subagents running on other models are counted in the real session cost.
// Fail-open: on any error print nothing and exit 0 — never break the line.
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

// $ per MTok: [input, output]. Cache: write 5m = 1.25x in, 1h = 2x in, read = 0.1x in.
const PRICES = [
  [/fable|mythos/, [10, 50]],
  [/opus/, [5, 25]],
  [/sonnet-5/, [2, 10]], // intro pricing through 2026-08-31 (standard 3/15)
  [/sonnet/, [3, 15]],
  [/haiku/, [1, 5]],
];
const CACHE_VERSION = 6;
const priceFor = (model) => (PRICES.find(([re]) => re.test(model)) ?? [null, [5, 25]])[1];
const shortName = (model) => model.replace(/^claude-/, "").replace(/-\d{8}$/, "");

const fmtTok = (n) =>
  n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? Math.round(n / 1e3) + "k" : String(n);

const fmtUsd = (n) =>
  n >= 10 ? "$" + n.toFixed(0) : n >= 0.1 ? "$" + n.toFixed(2) : "$" + n.toFixed(3);

let raw = "";
process.stdin.on("data", (c) => (raw += c));
process.stdin.on("end", () => {
  try {
    main(JSON.parse(raw));
  } catch {
    process.exit(0);
  }
});

function main(input) {
  const transcript = input.transcript_path;
  if (!transcript || !fs.existsSync(transcript)) return;

  const files = [transcript];
  const subDir = path.join(
    path.dirname(transcript),
    path.basename(transcript, ".jsonl"),
    "subagents"
  );
  if (fs.existsSync(subDir)) {
    for (const f of fs.readdirSync(subDir)) {
      if (f.endsWith(".jsonl")) files.push(path.join(subDir, f));
    }
  }

  // Signature cache: unchanged files -> reuse the previous line without re-parsing.
  let sig = `${CACHE_VERSION};`;
  for (const f of files) {
    const st = fs.statSync(f);
    sig += `${f}:${st.size}:${Math.round(st.mtimeMs)};`;
  }
  const cacheFile = path.join(
    os.tmpdir(),
    `claude-usage-${path.basename(transcript, ".jsonl")}.json`
  );
  try {
    const cached = JSON.parse(fs.readFileSync(cacheFile, "utf8"));
    if (cached.sig === sig) {
      process.stdout.write(cached.line);
      return;
    }
  } catch {}

  // Streaming appends several lines per message id with cumulative usage —
  // keep the last occurrence so nothing is double-counted.
  const byId = new Map();
  let seq = 0;
  for (const f of files) {
    let text;
    try {
      text = fs.readFileSync(f, "utf8");
    } catch {
      continue;
    }
    for (const lineRaw of text.split("\n")) {
      if (!lineRaw.includes('"usage"')) continue;
      let obj;
      try {
        obj = JSON.parse(lineRaw);
      } catch {
        continue;
      }
      const m = obj.message;
      if (!m || !m.usage || !m.model || m.model === "<synthetic>") continue;
      byId.set(m.id ?? `${f}#${seq++}`, { model: m.model, usage: m.usage });
    }
  }
  if (byId.size === 0) return;

  const perModel = new Map();
  for (const { model, usage } of byId.values()) {
    let s = perModel.get(model);
    if (!s) {
      s = { in: 0, out: 0, c5: 0, c1: 0, read: 0 };
      perModel.set(model, s);
    }
    s.in += usage.input_tokens ?? 0;
    s.out += usage.output_tokens ?? 0;
    s.read += usage.cache_read_input_tokens ?? 0;
    const cc = usage.cache_creation;
    if (cc && (cc.ephemeral_5m_input_tokens != null || cc.ephemeral_1h_input_tokens != null)) {
      s.c5 += cc.ephemeral_5m_input_tokens ?? 0;
      s.c1 += cc.ephemeral_1h_input_tokens ?? 0;
    } else {
      s.c5 += usage.cache_creation_input_tokens ?? 0;
    }
  }

  const parts = [];
  for (const [model, s] of perModel) {
    const [inP, outP] = priceFor(model);
    const cost =
      (s.in * inP + s.out * outP + s.c5 * inP * 1.25 + s.c1 * inP * 2 + s.read * inP * 0.1) / 1e6;
    const newTokens = s.in + s.out + s.c5 + s.c1;
    parts.push({ model: shortName(model), newTokens, cachedTokens: s.read, cost });
  }
  parts.sort((a, b) => b.cost - a.cost);

  const total = parts.reduce((a, p) => a + p.cost, 0);
  const usageLine = parts
    .map((p) => `${p.model} ${fmtTok(p.newTokens)} new/${fmtTok(p.cachedTokens)} cached`)
    .join(" + ");
  const costLine =
    parts.map((p) => `${p.model} ${fmtUsd(p.cost)}`).join(" + ") +
    (parts.length > 1 ? ` = ${fmtUsd(total)} billed` : " billed");
  const line = `${usageLine}\n${costLine}`;

  try {
    fs.writeFileSync(cacheFile, JSON.stringify({ sig, line }));
  } catch {}
  process.stdout.write(line);
}