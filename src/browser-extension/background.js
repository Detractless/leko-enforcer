import { COLLECTOR_ENDPOINT } from "./collector-config.js";
import { submitCapture } from "./collector-client.js";

const MAX_QUEUE = 20;
let queueWrites = Promise.resolve();
let flushTask = null;

function updateQueue(change) {
  queueWrites = queueWrites.catch(() => undefined).then(async () => {
    const { pendingCaptures = [] } = await chrome.storage.local.get("pendingCaptures");
    const next = await change(pendingCaptures);
    await chrome.storage.local.set({ pendingCaptures: next });
    return next;
  });
  return queueWrites;
}

async function collectionIsEnabled() {
  const { collectionEnabled = false } = await chrome.storage.local.get("collectionEnabled");
  return collectionEnabled === true;
}

async function flushQueue() {
  if (flushTask) return flushTask;
  flushTask = (async () => {
    if (!(await collectionIsEnabled())) return;
    while (true) {
      if (!(await collectionIsEnabled())) return;
      const { pendingCaptures = [] } = await chrome.storage.local.get("pendingCaptures");
      const capture = pendingCaptures[0];
      if (!capture) return;
      const { excludedHosts = [] } = await chrome.storage.local.get("excludedHosts");
      let captureHost = "";
      try { captureHost = new URL(capture.sourceUrl).hostname.toLowerCase(); } catch { /* malformed queue items are dropped */ }
      if (!captureHost || excludedHosts.includes(captureHost)) {
        await updateQueue((items) => items.filter((item) => item.captureId !== capture.captureId));
        continue;
      }
      try {
        await submitCapture(capture, COLLECTOR_ENDPOINT);
      } catch {
        await chrome.storage.local.set({ lastUploadError: "Upload failed; captures remain queued for retry." });
        return;
      }
      await chrome.storage.local.remove("lastUploadError");
      await updateReceipts((items) => {
        if (items.some((item) => item.captureId === capture.captureId)) return items;
        return [...items, { captureId: capture.captureId, deletionToken: capture.deletionToken }].slice(-500);
      });
      await updateQueue((items) => items.filter((item) => item.captureId !== capture.captureId));
    }
  })().finally(() => { flushTask = null; });
  return flushTask;
}

function updateReceipts(change) {
  queueWrites = queueWrites.catch(() => undefined).then(async () => {
    const { submittedReceipts = [] } = await chrome.storage.local.get("submittedReceipts");
    const next = await change(submittedReceipts);
    await chrome.storage.local.set({ submittedReceipts: next });
    return next;
  });
  return queueWrites;
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type === "queue-capture") {
    collectionIsEnabled().then(async (enabled) => {
      if (!enabled) return { accepted: false };
      const result = await updateQueue((items) => {
        if (items.some((item) => item.captureId === message.capture?.captureId)) return items;
        if (items.length >= MAX_QUEUE) throw new Error("local capture queue is full");
        return [...items, message.capture];
      });
      await flushQueue();
      return { accepted: result.some((item) => item.captureId === message.capture?.captureId) };
    }).then(sendResponse).catch(() => sendResponse({ accepted: false }));
    return true;
  }
  if (message?.type === "flush-queue") {
    flushQueue().then(() => sendResponse({ ok: true }));
    return true;
  }
  return false;
});

chrome.alarms.create("retry-captures", { periodInMinutes: 5 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "retry-captures") void flushQueue();
});
chrome.runtime.onStartup.addListener(() => void flushQueue());
