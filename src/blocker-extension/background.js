// Background Service Worker: Pipeline Daemon Coordinator & Tab State Manager

import { AdapterRegistry } from "./adapters/registry.js";

const DAEMON_URL = "http://127.0.0.1:8765/api/evaluate";
const DAEMON_TIMEOUT_MS = 8000;
const registry = new AdapterRegistry();
registry.init();

// In-memory cache for recent evaluations (URL -> Result)
const decisionCache = new Map();
const inFlightRequests = new Map(); // URL -> Promise
const CACHE_TTL_MS = 60000; // 1 minute
const lastNavigatedUrls = new Map(); // tabId -> normalizedUrl

function normalizeUrl(rawUrl) {
  try {
    const u = new URL(rawUrl);
    return `${u.origin}${u.pathname}${u.search}`;
  } catch (e) {
    return (rawUrl || "").split("#")[0];
  }
}

async function fetchImageAsBase64(imageUrl) {
  try {
    const res = await fetch(imageUrl);
    if (!res.ok) return null;
    const blob = await res.blob();
    return new Promise((resolve) => {
      const reader = new FileReader();
      reader.onloadend = () => resolve(reader.result);
      reader.onerror = () => resolve(null);
      reader.readAsDataURL(blob);
    });
  } catch (err) {
    return null;
  }
}

function shouldFetchImage(url, wordCount, hasImages) {
  if (!hasImages) return false;
  if (wordCount < 30) return true;
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.toLowerCase();
    const path = parsed.pathname.toLowerCase();
    const mediaCues = ["video", "tube", "watch", "porn", "xxx", "hentai", "stream", "movie", "media"];
    if (mediaCues.some((cue) => host.includes(cue) || path.includes(cue))) {
      return true;
    }
  } catch (e) {}
  return false;
}

async function evaluateWithDaemon(payload) {
  if (!payload.force_evaluate) {
    const cached = decisionCache.get(payload.url);
    if (cached && (Date.now() - cached.timestamp < CACHE_TTL_MS)) {
      return cached.result;
    }
    if (inFlightRequests.has(payload.url)) {
      return inFlightRequests.get(payload.url);
    }
  }

  const exec = async () => {
    let imageB64 = null;
    const words = (payload.text || "").trim().split(/\s+/).filter(Boolean);
    const hasImages = Array.isArray(payload.images) && payload.images.length > 0;

    if (shouldFetchImage(payload.url, words.length, hasImages)) {
      imageB64 = await fetchImageAsBase64(payload.images[0]);
    }

    const body = {
      url: payload.url,
      title: payload.title,
      html: payload.html,
      text: payload.text,
      dom_method: payload.dom_method,
      page_type: payload.page_type || "webpage",
      chunks: payload.chunks || null,
      total_extracted: payload.total_extracted || 0,
      image_b64: imageB64,
      force_evaluate: Boolean(payload.force_evaluate)
    };

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), DAEMON_TIMEOUT_MS);
    let res;
    try {
      res = await fetch(DAEMON_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal
      });
    } finally {
      clearTimeout(timeoutId);
    }

    if (!res.ok) {
      throw new Error(`Daemon returned HTTP ${res.status}`);
    }

    const result = await res.json();
    if (result.verdict !== "PENDING") {
      decisionCache.set(payload.url, { result, timestamp: Date.now() });
    }

    return result;
  };

  const promise = exec().finally(() => {
    inFlightRequests.delete(payload.url);
  });

  if (!payload.force_evaluate) {
    inFlightRequests.set(payload.url, promise);
  }

  return promise;
}

// Runtime message listener
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "EVALUATE_PAGE") {
    evaluateWithDaemon(message.payload)
      .then((res) => sendResponse(res))
      .catch((err) => {
        console.error("[Leko Blocker Background] Evaluation error (failing closed):", err);
        sendResponse({ verdict: "BLOCK", policy_triggered: "daemon_unavailable", error: String(err) });
      });
    return true; // Keep channel open for async response
  }

  if (message.type === "UPDATE_ADAPTERS") {
    registry.mergeRemoteManifest(message.url)
      .then((res) => sendResponse(res))
      .catch((err) => sendResponse({ success: false, error: String(err) }));
    return true;
  }
});

// Watch SPA history navigation on top-level frames (frameId === 0)
chrome.webNavigation.onHistoryStateUpdated.addListener((details) => {
  if (details.frameId !== 0) return;
  const normUrl = normalizeUrl(details.url);
  const lastNorm = lastNavigatedUrls.get(details.tabId);
  if (lastNorm === normUrl) return; // Deduplicate fragment/anchor updates
  lastNavigatedUrls.set(details.tabId, normUrl);
  chrome.tabs.sendMessage(details.tabId, {
    type: "SPA_NAVIGATION",
    url: details.url
  }).catch(() => {});
});

// Watch standard tab updates
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.url && /^https?:\/\//i.test(changeInfo.url)) {
    const normUrl = normalizeUrl(changeInfo.url);
    if (lastNavigatedUrls.get(tabId) === normUrl) return;
    lastNavigatedUrls.set(tabId, normUrl);
    chrome.tabs.sendMessage(tabId, { type: "TAB_URL_CHANGED", url: changeInfo.url }).catch(() => {});
  }
});

// Clean up tab maps
chrome.tabs.onRemoved.addListener((tabId) => {
  lastNavigatedUrls.delete(tabId);
});
