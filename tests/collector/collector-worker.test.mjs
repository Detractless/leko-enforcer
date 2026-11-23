import test from "node:test";
import assert from "node:assert/strict";
import worker from "../src/collector-worker/worker.js";

const origin = "chrome-extension://test-extension-id";
const capture = {
  captureId: "123e4567-e89b-42d3-a456-426614174000",
  sourceUrl: "https://example.com/page?utm_source=feed",
  text: "Readable page text",
  capturedAt: "2026-11-12T10:00:00.000Z",
  extractorVersion: "dom-readable-text-v1",
  consentVersion: "collection-opt-in-v1",
};

function baseEnv() {
  return {
    GITHUB_APP_ID: "42",
    GITHUB_APP_PRIVATE_KEY: "not-used-by-invalid-input",
    GITHUB_INSTALLATION_ID: "10",
    GITHUB_OWNER: "Detractless",
    GITHUB_REPOSITORY: "Website-Analysis-Engine",
    EXTENSION_ORIGIN: origin,
    CAPTURE_LIMITER: { limit: async () => ({ success: true }) },
  };
}

function request(method, path, body, extra = {}) {
  const headers = {
    Origin: origin,
    "CF-Connecting-IP": "192.0.2.1",
    ...extra,
  };
  if (body !== undefined) headers["Content-Type"] = "application/json";
  return new Request(`https://collector.example${path}`, {
    method,
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
}

async function testPrivateKey() {
  const pair = await crypto.subtle.generateKey({
    name: "RSASSA-PKCS1-v1_5",
    modulusLength: 1024,
    publicExponent: new Uint8Array([1, 0, 1]),
    hash: "SHA-256",
  }, true, ["sign", "verify"]);
  const der = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
  let binary = "";
  for (const byte of der) binary += String.fromCharCode(byte);
  return `-----BEGIN PRIVATE KEY-----\n${btoa(binary)}\n-----END PRIVATE KEY-----`;
}

test("rejects unknown browser origins", async () => {
  const response = await worker.fetch(new Request("https://collector.example/v1/captures", {
    method: "POST",
    headers: { Origin: "https://other.example" },
  }), baseEnv());
  assert.equal(response.status, 403);
});

test("answers the extension preflight request", async () => {
  const response = await worker.fetch(request("OPTIONS", "/v1/captures"), baseEnv());
  assert.equal(response.status, 204);
  assert.equal(response.headers.get("Access-Control-Allow-Origin"), origin);
});

test("checks rate limits before accepting a capture", async () => {
  const env = baseEnv();
  env.CAPTURE_LIMITER.limit = async () => ({ success: false });
  const response = await worker.fetch(request("POST", "/v1/captures", capture, {
    "Idempotency-Key": capture.captureId,
    "X-Deletion-Token": "a".repeat(64),
  }), env);
  assert.equal(response.status, 429);
});

test("rejects records with a mismatched idempotency key before GitHub auth", async () => {
  const response = await worker.fetch(request("POST", "/v1/captures", capture, {
    "Idempotency-Key": "different",
    "X-Deletion-Token": "a".repeat(64),
  }), baseEnv());
  assert.equal(response.status, 400);
  assert.match((await response.json()).error, /idempotency_key/);
});

test("rejects oversized request bodies before writing to GitHub", async () => {
  const response = await worker.fetch(request("POST", "/v1/captures", { ...capture, text: "x".repeat(530_000) }, {
    "Idempotency-Key": capture.captureId,
    "X-Deletion-Token": "a".repeat(64),
  }), baseEnv());
  assert.equal(response.status, 413);
});

test("stores a capture through the GitHub App and avoids duplicate commits on retry", async () => {
  const env = { ...baseEnv(), GITHUB_APP_PRIVATE_KEY: await testPrivateKey() };
  const originalFetch = globalThis.fetch;
  let savedContent = "";
  let putCount = 0;
  globalThis.fetch = async (url, options = {}) => {
    if (String(url).endsWith("/access_tokens")) return new Response(JSON.stringify({ token: "installation-token" }), { status: 201 });
    if (options.method === "PUT") {
      putCount += 1;
      savedContent = JSON.parse(options.body).content;
      return new Response(JSON.stringify({ content: {} }), { status: putCount === 1 ? 201 : 422 });
    }
    if (!options.method || options.method === "GET") {
      return new Response(JSON.stringify({ content: savedContent }), { status: 200 });
    }
    throw new Error(`unexpected GitHub request ${options.method}`);
  };
  try {
    for (let attempt = 0; attempt < 2; attempt += 1) {
      const response = await worker.fetch(request("POST", "/v1/captures", capture, {
        "Idempotency-Key": capture.captureId,
        "X-Deletion-Token": "b".repeat(64),
      }), env);
      assert.equal(response.status, 200);
      assert.equal((await response.json()).status, attempt ? "duplicate" : "stored");
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
  assert.equal(putCount, 2);
});

test("deletes only when the submitted deletion receipt matches", async () => {
  const deletionToken = "c".repeat(64);
  const tokenHashBytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(deletionToken));
  const tokenHash = [...new Uint8Array(tokenHashBytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  const stored = { ...capture, deletionTokenHash: tokenHash };
  const encoded = btoa(unescape(encodeURIComponent(JSON.stringify(stored, null, 2) + "\n")));
  const originalFetch = globalThis.fetch;
  let deleteCalls = 0;
  globalThis.fetch = async (_url, options = {}) => {
    if (String(_url).endsWith("/access_tokens")) return new Response(JSON.stringify({ token: "installation-token" }), { status: 201 });
    if (options.method === "DELETE") {
      deleteCalls += 1;
      return new Response(JSON.stringify({ commit: {} }), { status: 200 });
    }
    return new Response(JSON.stringify({ content: encoded, sha: "file-sha" }), { status: 200 });
  };
  try {
    const response = await worker.fetch(request("DELETE", `/v1/captures/${capture.captureId}`, undefined, {
      "X-Deletion-Token": deletionToken,
    }), { ...baseEnv(), GITHUB_APP_PRIVATE_KEY: await testPrivateKey() });
    assert.equal(response.status, 204);
  } finally {
    globalThis.fetch = originalFetch;
  }
  assert.equal(deleteCalls, 1);
});
