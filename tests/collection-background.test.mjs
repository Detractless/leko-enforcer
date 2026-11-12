import test from "node:test";
import assert from "node:assert/strict";

const storage = {};
let messageListener;
globalThis.chrome = {
  storage: {
    local: {
      async get(keys) {
        const list = Array.isArray(keys) ? keys : [keys];
        return Object.fromEntries(list.filter((key) => key in storage).map((key) => [key, storage[key]]));
      },
      async set(values) { Object.assign(storage, values); },
      async remove(keys) { for (const key of Array.isArray(keys) ? keys : [keys]) delete storage[key]; },
    },
  },
  runtime: {
    onMessage: { addListener(listener) { messageListener = listener; } },
    onStartup: { addListener() {} },
  },
  alarms: { create() {}, onAlarm: { addListener() {} } },
};

await import(`../src/browser-extension/background.js?test=${Date.now()}`);

function send(message) {
  return new Promise((resolve) => {
    const keepOpen = messageListener(message, {}, resolve);
    if (!keepOpen) resolve(undefined);
  });
}

test("does not queue a page when collection has never been enabled", async () => {
  const response = await send({ type: "queue-capture", capture: { captureId: "off" } });
  assert.deepEqual(response, { accepted: false });
  assert.equal(storage.pendingCaptures, undefined);
});

test("queues an opted-in capture locally while the collector endpoint is unconfigured", async () => {
  storage.collectionEnabled = true;
  const capture = {
    captureId: "123e4567-e89b-42d3-a456-426614174000",
    deletionToken: "a".repeat(64),
    sourceUrl: "https://example.com",
    text: "Readable text",
  };
  const response = await send({ type: "queue-capture", capture });
  assert.deepEqual(response, { accepted: true });
  assert.deepEqual(storage.pendingCaptures, [capture]);
  assert.match(storage.lastUploadError, /remain queued/);
});
