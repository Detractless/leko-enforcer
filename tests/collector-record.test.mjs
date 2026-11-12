import test from "node:test";
import assert from "node:assert/strict";
import { cleanSourceUrl, validateCapture } from "../src/collector-worker/records.js";

const capture = {
  captureId: "123e4567-e89b-42d3-a456-426614174000",
  sourceUrl: "https://example.com/article?topic=memory&utm_source=feed&session_token=secret#comments",
  text: "Readable page text",
  capturedAt: "2026-11-12T10:00:00.000Z",
  extractorVersion: "dom-readable-text-v1",
  consentVersion: "collection-opt-in-v1",
  adapter: { id: "example", version: "1" },
};

test("removes URL fragments, tracking parameters, and credential-like query keys", () => {
  assert.equal(cleanSourceUrl(capture.sourceUrl), "https://example.com/article?topic=memory");
  assert.equal(cleanSourceUrl("https://user:pass@example.com/path"), "https://example.com/path");
});

test("rejects non-web source URLs", () => {
  assert.throws(() => cleanSourceUrl("javascript:alert(1)"), /HTTP or HTTPS/);
});

test("validates and normalizes the record without HTML or unknown fields", () => {
  const record = validateCapture({ ...capture, ignored: "not stored" });
  assert.equal(record.sourceUrl, "https://example.com/article?topic=memory");
  assert.deepEqual(record.adapter, { id: "example", version: "1" });
  assert.equal("ignored" in record, false);
});

test("rejects empty and oversized page text", () => {
  assert.throws(() => validateCapture({ ...capture, text: " " }), /text must contain/);
  assert.throws(() => validateCapture({ ...capture, text: "x".repeat(100_001) }), /text must contain/);
});
