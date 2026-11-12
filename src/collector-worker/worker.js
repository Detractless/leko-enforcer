import { deleteCapture, writeCapture } from "./github-app.js";
import { validateCapture } from "./records.js";

const MAX_BODY_BYTES = 512 * 1024;

function json(body, status, headers = {}) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store", ...headers },
  });
}

function corsHeaders(origin, env) {
  if (!origin || origin !== env.EXTENSION_ORIGIN) return null;
  return {
    "Access-Control-Allow-Origin": origin,
    "Access-Control-Allow-Methods": "POST, DELETE, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Idempotency-Key, X-Deletion-Token",
    "Access-Control-Max-Age": "600",
    Vary: "Origin",
  };
}

async function readJson(request) {
  const declaredLength = Number(request.headers.get("Content-Length") || 0);
  if (declaredLength > MAX_BODY_BYTES) throw new RangeError("request body is too large");
  if (!request.body) throw new TypeError("request body is required");
  const reader = request.body.getReader();
  const chunks = [];
  let size = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_BODY_BYTES) {
      await reader.cancel();
      throw new RangeError("request body is too large");
    }
    chunks.push(value);
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  try {
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw new SyntaxError("request body must be valid UTF-8 JSON");
  }
}

function hasGitHubConfig(env) {
  return [
    "GITHUB_APP_ID",
    "GITHUB_APP_PRIVATE_KEY",
    "GITHUB_INSTALLATION_ID",
    "GITHUB_OWNER",
    "GITHUB_REPOSITORY",
  ].every((key) => typeof env[key] === "string" && env[key].length > 0);
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const deleteMatch = url.pathname.match(/^\/v1\/captures\/([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})$/i);
    const deletePath = deleteMatch && ["DELETE", "OPTIONS"].includes(request.method);
    if (url.pathname !== "/v1/captures" && !deletePath) {
      return json({ error: "not_found" }, 404);
    }
    const cors = corsHeaders(request.headers.get("Origin"), env);
    if (!cors) return json({ error: "origin_not_allowed" }, 403);
    if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: cors });
    if (!(["POST", "DELETE"].includes(request.method))) return json({ error: "method_not_allowed" }, 405, { Allow: "POST, DELETE, OPTIONS", ...cors });
    if (!env.CAPTURE_LIMITER) return json({ error: "collector_not_configured" }, 503, cors);
    const clientIp = request.headers.get("CF-Connecting-IP");
    if (!clientIp) return json({ error: "rate_limit_unavailable" }, 503, cors);
    let limit;
    try {
      limit = await env.CAPTURE_LIMITER.limit({ key: clientIp });
    } catch {
      return json({ error: "rate_limit_unavailable" }, 503, cors);
    }
    if (!limit.success) return json({ error: "rate_limited" }, 429, { ...cors, "Retry-After": "60" });
    if (!hasGitHubConfig(env)) return json({ error: "collector_not_configured" }, 503, cors);

    if (request.method === "DELETE") {
      const deletionToken = request.headers.get("X-Deletion-Token") || "";
      if (!/^[0-9a-f]{64}$/i.test(deletionToken)) return json({ error: "invalid_deletion_token" }, 400, cors);
      const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(deletionToken));
      const tokenHash = [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
      try {
        await deleteCapture(env, deleteMatch[1].toLowerCase(), tokenHash);
        return new Response(null, { status: 204, headers: cors });
      } catch {
        return json({ error: "capture_delete_failed" }, 502, cors);
      }
    }
    if (!request.headers.get("Content-Type")?.toLowerCase().startsWith("application/json")) {
      return json({ error: "content_type_must_be_json" }, 415, cors);
    }

    let record;
    try {
      record = validateCapture(await readJson(request));
    } catch (error) {
      const status = error instanceof RangeError ? 413 : 400;
      return json({ error: error.message }, status, cors);
    }
    if (request.headers.get("Idempotency-Key") !== record.captureId) {
      return json({ error: "idempotency_key_must_match_capture_id" }, 400, cors);
    }
    const deletionToken = request.headers.get("X-Deletion-Token") || "";
    if (!/^[0-9a-f]{64}$/i.test(deletionToken)) return json({ error: "invalid_deletion_token" }, 400, cors);
    const tokenBytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(deletionToken));
    const tokenHash = [...new Uint8Array(tokenBytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    try {
      const result = await writeCapture(env, record, tokenHash);
      return json({ captureId: record.captureId, status: result.duplicate ? "duplicate" : "stored" }, 200, cors);
    } catch {
      return json({ error: "capture_write_failed" }, 502, cors);
    }
  },
};
