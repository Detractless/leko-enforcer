// lockctl commands: lock, unlock, relock, status, recover.
// Each returns { code, out: string[], err: string[] } and never prints; the CLI
// prints. Exit codes: 0 done / healthy, 1 refused, failed, or problems found.
//
// State machine per path: (none) -lock-> locked -unlock-> unlocked -relock-> locked.
// `lock` only creates an entry. `unlock` needs the user's words and a purpose.
// `relock` needs a verify command that exits 0; if it fails the entry stays
// unlocked and records a `debt`. Every transition is appended to `history`.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { hashState, readRecords, sha256File, writeRecords } from "./locks.mjs";
import { toRel } from "./paths.mjs";

const done = (out) => ({ code: 0, out, err: [] });
const refuse = (...err) => ({ code: 1, out: [], err });
const TRIVIAL_VERIFY = /^\s*(true|:|exit\s+0)\s*;?\s*$/;
const now = () => new Date().toISOString();
const short = (sha) => String(sha).slice(0, 12);

function need(opts, names) {
  const missing = names.filter((n) => typeof opts[n] !== "string" || opts[n].trim() === "");
  return missing.length ? refuse(`missing required option(s): ${missing.map((n) => `--${n}`).join(", ")}`) : null;
}

function target(ctx, rawPath) {
  if (!rawPath) return { bad: refuse("missing <path>") };
  const abs = path.resolve(ctx.cwd, rawPath);
  const rel = toRel(ctx.root, abs);
  if (rel === null) return { bad: refuse(`${rawPath} is outside the project (${ctx.root})`) };
  return { abs, rel };
}

function load(ctx) {
  const rec = readRecords(ctx.recordsFile);
  if (rec.state === "corrupt") {
    return { bad: refuse(`refusing to continue: lock records ${ctx.recordsRel} are corrupt (${rec.problems[0]}). Nothing was changed. Run: lockctl recover`) };
  }
  return { data: rec.state === "ok" ? rec.data : { locks: [] }, exists: rec.state === "ok" };
}

function save(ctx, data) {
  writeRecords(ctx.recordsFile, data);
}

export function lockCmd(ctx, rawPath, opts) {
  const miss = need(opts, ["reason", "evidence"]);
  const t = target(ctx, rawPath);
  if (t.bad) return t.bad;
  if (miss) return miss;
  let st;
  try { st = fs.statSync(t.abs); } catch { return refuse(`${t.rel} does not exist; only an existing file can be locked`); }
  if (!st.isFile()) return refuse(`${t.rel} is not a regular file`);
  const l = load(ctx);
  if (l.bad) return l.bad;
  const prior = l.data.locks.find((e) => e.path === t.rel);
  if (prior) {
    const note = prior.state === "locked" && hashState(ctx.root, prior) === "stale" ? " Its record is STALE (the file changed while locked); that cannot be re-recorded by lock. Report it, and use unlock with the user's authorization." : "";
    return refuse(`${t.rel} already has a lock entry (state ${prior.state}).${prior.state === "unlocked" ? " Finish it with relock --verify." : ""}${note}`);
  }
  const sha = sha256File(t.abs);
  const at = now();
  l.data.locks.push({
    path: t.rel, sha256: sha, state: "locked", reason: opts.reason, evidence: opts.evidence, authorizedBy: null, updatedAt: at,
    history: [{ at, event: "lock", sha256: sha }],
  });
  save(ctx, l.data);
  return done([`locked ${t.rel} (sha256 ${short(sha)})`]);
}

export function unlockCmd(ctx, rawPath, opts) {
  const t = target(ctx, rawPath);
  if (t.bad) return t.bad;
  const miss = need(opts, ["authorized-by", "purpose"]);
  if (miss) return refuse(...miss.err, "unlock needs the user's own words (--authorized-by) and the stated purpose (--purpose); without both nothing changes");
  const l = load(ctx);
  if (l.bad) return l.bad;
  const entry = l.data.locks.find((e) => e.path === t.rel);
  if (!entry) return refuse(`${t.rel} has no lock entry; nothing to unlock`);
  if (entry.state === "unlocked") return refuse(`${t.rel} is already unlocked (purpose: ${entry.purpose || "unknown"}). Finish it with relock --verify.`);
  const stale = hashState(ctx.root, entry) !== "ok";
  const at = now();
  Object.assign(entry, {
    state: "unlocked", authorizedBy: opts["authorized-by"], purpose: opts.purpose, unlockedAt: at, updatedAt: at,
    unlockedFromSha256: entry.sha256, staleAtUnlock: stale,
  });
  entry.history = [...(entry.history || []), { at, event: "unlock", authorizedBy: opts["authorized-by"], purpose: opts.purpose, staleAtUnlock: stale }];
  save(ctx, l.data);
  const out = [`unlocked ${t.rel} for: ${opts.purpose}`, `run relock --verify "<command>" when the change is done; the unlock stays visible until then`];
  if (stale) out.push(`warning: ${t.rel} had already changed while it was locked (record was stale)`);
  return done(out);
}

function tail(text) {
  const t = String(text || "").trim();
  return t.length > 400 ? `...${t.slice(-400)}` : t;
}

export function relockCmd(ctx, rawPath, opts) {
  const t = target(ctx, rawPath);
  if (t.bad) return t.bad;
  const miss = need(opts, ["reason", "evidence", "verify"]);
  if (miss) return miss;
  if (TRIVIAL_VERIFY.test(opts.verify)) return refuse(`--verify "${opts.verify}" checks nothing; give a command that exercises the change`);
  const l = load(ctx);
  if (l.bad) return l.bad;
  const entry = l.data.locks.find((e) => e.path === t.rel);
  if (!entry) return refuse(`${t.rel} has no lock entry; use lock first`);
  if (entry.state !== "unlocked") return refuse(`${t.rel} is not unlocked (state ${entry.state}); there is nothing to relock`);
  const sha = sha256File(t.abs);
  if (sha === null) return refuse(`${t.rel} is missing; cannot relock`);
  const seconds = Number(opts.timeout) > 0 ? Number(opts.timeout) : 120;
  const r = spawnSync("sh", ["-c", opts.verify], { cwd: ctx.root, encoding: "utf8", timeout: seconds * 1000, maxBuffer: 5 * 1024 * 1024 });
  const at = now();
  const code = r.status;
  if (code !== 0) {
    const why = r.error && r.error.code === "ETIMEDOUT" ? `timed out after ${seconds}s` : `exit ${code === null ? `signal ${r.signal}` : code}`;
    entry.debt = { at, command: opts.verify, exitCode: code, summary: why, output: tail(`${r.stdout}\n${r.stderr}`) };
    entry.updatedAt = at;
    entry.history = [...(entry.history || []), { at, event: "relock-failed", command: opts.verify, summary: why }];
    save(ctx, l.data);
    return { code: 1, out: [], err: [`relock FAILED for ${t.rel}: verify command ${why}. It stays UNLOCKED and the debt is recorded (see status). Not verified.`, tail(`${r.stdout}\n${r.stderr}`)].filter(Boolean) };
  }
  delete entry.debt;
  Object.assign(entry, {
    state: "locked", sha256: sha, reason: opts.reason, evidence: opts.evidence, authorizedBy: null, updatedAt: at,
    verified: { at, command: opts.verify, exitCode: 0, output: tail(`${r.stdout}\n${r.stderr}`) },
  });
  entry.history = [...(entry.history || []), { at, event: "relock", sha256: sha, command: opts.verify }];
  save(ctx, l.data);
  return done([`relocked ${t.rel} (sha256 ${short(sha)}); verify command exited 0`]);
}

/** Facts about every entry; shared with the audit. */
export function lockFacts(root, data) {
  return data.locks.map((e) => {
    const hash = hashState(root, e);
    const pending = e.state === "unlocked";
    return { entry: e, hash, problem: (!pending && hash !== "ok") || Boolean(e.debt) };
  });
}

export function statusCmd(ctx, rawPath, opts) {
  const rec = readRecords(ctx.recordsFile);
  if (rec.state === "corrupt") return { code: 1, out: [], err: [`lock records ${ctx.recordsRel} are CORRUPT: ${rec.problems[0]}. Run: lockctl recover`] };
  if (rec.state === "missing") return done([`no lock records yet (${ctx.recordsRel} does not exist)`]);
  let facts = lockFacts(ctx.root, rec.data);
  if (rawPath) {
    const t = target(ctx, rawPath);
    if (t.bad) return t.bad;
    facts = facts.filter((f) => f.entry.path === t.rel);
  }
  if (opts.json) {
    const rows = facts.map((f) => ({ path: f.entry.path, state: f.entry.state, check: f.entry.state === "unlocked" ? "pending" : f.hash === "ok" ? "ok" : f.hash, debt: f.entry.debt || null }));
    return { code: facts.some((f) => f.problem) ? 1 : 0, out: [JSON.stringify(rows)], err: [] };
  }
  const out = facts.map(({ entry: e, hash }) => {
    const check = e.state === "unlocked"
      ? `UNFINISHED unlock (purpose: ${e.purpose || "?"}; hash ${hash === "ok" ? "unchanged since lock" : hash === "stale" ? "differs from the record" : "file missing"}); relock pending`
      : hash;
    const head = `${e.state.padEnd(8)} ${e.path}  ${check}  sha256 ${short(e.sha256)}`;
    const extra = [e.reason ? `    reason: ${e.reason}` : null, e.debt ? `    DEBT: relock verify failed at ${e.debt.at}: ${e.debt.summary} (${e.debt.command})` : null].filter(Boolean);
    return [head, ...extra].join("\n");
  });
  if (!out.length) out.push("no lock entries");
  return { code: facts.some((f) => f.problem) ? 1 : 0, out, err: [] };
}

export function recoverCmd(ctx) {
  const rec = readRecords(ctx.recordsFile);
  if (rec.state !== "corrupt") return refuse(`lock records are ${rec.state === "ok" ? "valid" : "absent"}; nothing to recover`);
  const bak = readRecords(`${ctx.recordsFile}.bak`);
  if (bak.state !== "ok") return refuse(`cannot recover: no usable ${ctx.recordsRel}.bak (${bak.state === "missing" ? "missing" : bak.problems[0]}). The corrupt file was left untouched; repair or replace it by hand.`);
  const aside = `${ctx.recordsFile}.corrupt-${Date.now()}`;
  const tmp = `${ctx.recordsFile}.tmp-${process.pid}-${Date.now()}`;
  fs.copyFileSync(`${ctx.recordsFile}.bak`, tmp);
  fs.renameSync(ctx.recordsFile, aside);
  fs.renameSync(tmp, ctx.recordsFile);
  return done([`restored ${ctx.recordsRel} from ${ctx.recordsRel}.bak (${bak.data.locks.length} entries)`, `corrupt file moved to ${path.relative(ctx.root, aside)}`, "changes made after that backup are not in the restored file; check status"]);
}
