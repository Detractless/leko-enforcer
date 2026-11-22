import { COLLECTOR_ENDPOINT, isCollectorConfigured } from "./collector-config.js";
import { deleteSubmittedCapture } from "./collector-client.js";

const toggleButton = document.querySelector("#toggle");
const excludeButton = document.querySelector("#exclude");
const clearButton = document.querySelector("#clear");
const deleteSentButton = document.querySelector("#delete-sent");
const statusText = document.querySelector("#status");
const noticeText = document.querySelector("#notice");
const queueText = document.querySelector("#queue");
const submittedText = document.querySelector("#submitted");

async function activeTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  return tab;
}

async function activeHost() {
  const url = new URL((await activeTab())?.url || "about:blank");
  return ["http:", "https:"].includes(url.protocol) ? url.hostname.toLowerCase() : null;
}

async function refresh() {
  const settings = await chrome.storage.local.get(["collectionEnabled", "excludedHosts", "pendingCaptures", "submittedReceipts", "lastUploadError"]);
  const enabled = settings.collectionEnabled === true;
  const host = await activeHost();
  excludeButton.disabled = !host;
  const excluded = host && (settings.excludedHosts || []).includes(host);
  toggleButton.textContent = enabled ? "Turn collection off" : "Turn collection on";
  statusText.textContent = !enabled ? "Collection is off." : excluded ? "Collection is on; this site is excluded." : "Collection is on for this site.";
  queueText.textContent = `${(settings.pendingCaptures || []).length} captures waiting`;
  submittedText.textContent = `${(settings.submittedReceipts || []).length} sent captures`;
  excludeButton.textContent = excluded ? "Allow this site again" : "Never collect this site";
  if (!isCollectorConfigured(COLLECTOR_ENDPOINT)) {
    noticeText.textContent = "Collector setup is incomplete; captures stay in this browser until configured.";
  } else {
    noticeText.textContent = settings.lastUploadError || "";
  }
}

toggleButton.addEventListener("click", async () => {
  const { collectionEnabled = false } = await chrome.storage.local.get("collectionEnabled");
  await chrome.storage.local.set({ collectionEnabled: !collectionEnabled });
  if (collectionEnabled) statusText.textContent = "Collection is off.";
  noticeText.textContent = "";
  await refresh();
  if (!collectionEnabled) {
    const tab = await activeTab();
    if (tab?.id != null) chrome.tabs.sendMessage(tab.id, { type: "capture-current-page" }).catch(() => undefined);
    chrome.runtime.sendMessage({ type: "flush-queue" });
  }
});

excludeButton.addEventListener("click", async () => {
  const host = await activeHost();
  if (!host) return;
  const { excludedHosts = [] } = await chrome.storage.local.get("excludedHosts");
  const excluded = excludedHosts.includes(host);
  const next = excluded ? excludedHosts.filter((item) => item !== host) : [...excludedHosts, host];
  await chrome.storage.local.set({ excludedHosts: next });
  await refresh();
});

clearButton.addEventListener("click", async () => {
  await chrome.storage.local.set({ pendingCaptures: [], lastUploadError: "" });
  await refresh();
});

deleteSentButton.addEventListener("click", async () => {
  const { submittedReceipts = [] } = await chrome.storage.local.get("submittedReceipts");
  if (!submittedReceipts.length || !confirm(`Delete all ${submittedReceipts.length} sent captures from the private research inbox?`)) return;
  const remaining = [];
  let deleted = 0;
  for (let index = 0; index < submittedReceipts.length; index += 1) {
    const receipt = submittedReceipts[index];
    if (deleted >= 15) {
      remaining.push(...submittedReceipts.slice(index));
      break;
    }
    try {
      await deleteSubmittedCapture(receipt.captureId, receipt.deletionToken, COLLECTOR_ENDPOINT);
      deleted += 1;
    } catch {
      remaining.push(...submittedReceipts.slice(index));
      break;
    }
  }
  await chrome.storage.local.set({ submittedReceipts: remaining });
  noticeText.textContent = remaining.length ? `Deleted ${deleted}; ${remaining.length} remain. Click again later to continue.` : `Deleted ${deleted} sent captures.`;
  await refresh();
});

void refresh();
