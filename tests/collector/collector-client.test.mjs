import test from "node:test";
import assert from "node:assert/strict";
import { deleteSubmittedCapture, submitCapture } from "../src/browser-extension/collector-client.js";

const endpoint = "https://collector.example.workers.dev/v1/captures";
const capture = {
  captureId: "123e4567-e89b-42d3-a456-426614174000",
  deletionToken: "a".repeat(64),
  sourceUrl: "https://example.com",
  text: "Page text",
};

test("uploads record with an idempotency key and deletion receipt header", async () => {
  let request;
  const result = await submitCapture(capture, endpoint, async (_url, options) => {
    request = options;
    return new Response(JSON.stringify({ status: "stored" }), { status: 200 });
  });
  assert.equal(request.headers["Idempotency-Key"], capture.captureId);
  assert.equal(request.headers["X-Deletion-Token"], capture.deletionToken);
  assert.equal(JSON.parse(request.body).deletionToken, undefined);
  assert.equal(result.status, "stored");
});

test("does not send to an unconfigured endpoint", async () => {
  await assert.rejects(submitCapture(capture, "https://REPLACE_WITH_YOUR_WORKER.workers.dev/v1/captures"), /not configured/);
});

test("sends a deletion request with the saved receipt", async () => {
  let requestUrl;
  let requestOptions;
  await deleteSubmittedCapture(capture.captureId, capture.deletionToken, endpoint, async (url, options) => {
    requestUrl = url;
    requestOptions = options;
    return new Response(null, { status: 204 });
  });
  assert.equal(requestUrl, `${endpoint}/${capture.captureId}`);
  assert.equal(requestOptions.headers["X-Deletion-Token"], capture.deletionToken);
});
