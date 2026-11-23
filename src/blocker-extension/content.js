// Content Script: DOM Extraction, Route Normalization, Curtain Management & Chunk Redaction

function normalizeUrl(u) {
  try {
    const parsed = new URL(u, window.location.href);
    return parsed.origin + parsed.pathname + parsed.search;
  } catch (_) {
    return (u || "").split("#")[0];
  }
}

let currentUrl = window.location.href;
let currentNormalizedUrl = normalizeUrl(window.location.href);
let retryAttempts = 0;
const MAX_RETRIES = 5;
let retryTimeout = null;
let spaStabilityTimeout = null;
let spaObserver = null;
let currentNavigationId = 0;

let shadowMode = false;

function isVisible(el) {
  if (!el || !el.getBoundingClientRect) return false;
  const rect = el.getBoundingClientRect();
  return (
    rect.width > 80 && rect.height > 80 &&
    rect.top < window.innerHeight * 2 &&
    window.getComputedStyle(el).display !== "none" &&
    window.getComputedStyle(el).visibility !== "hidden"
  );
}

function haltAndStripMedia() {
  const media = document.querySelectorAll("video, audio");
  media.forEach((el) => {
    try { el.pause(); } catch (e) {}
    try { el.muted = true; } catch (e) {}
    try { el.removeAttribute("src"); el.load && el.load(); } catch (e) {}
    try { el.remove(); } catch (e) {}
  });
}

function lockCurtain(policy) {
  haltAndStripMedia();
  if (window.__WAE_CURTAIN__) window.__WAE_CURTAIN__.showBlock(policy || "blocked");
}

function extractImagesFromNode(rootNode, candidateUrls) {
  if (!rootNode || candidateUrls.length >= 5) return;
  const els = rootNode.querySelectorAll ? rootNode.querySelectorAll("img, [role=\"img\"]") : [];
  for (const img of els) {
    if (isVisible(img)) {
      const src = img.currentSrc || img.src || img.getAttribute("data-src") || img.getAttribute("srcset");
      if (src && !src.startsWith("data:image/svg") && /^https?:\/\//i.test(src)) {
        candidateUrls.push(src.split(" ")[0]);
        if (candidateUrls.length >= 5) return;
      }
    }
  }

  const allElements = rootNode.querySelectorAll ? rootNode.querySelectorAll("*") : [];
  for (const el of allElements) {
    if (el.shadowRoot) {
      extractImagesFromNode(el.shadowRoot, candidateUrls);
      if (candidateUrls.length >= 5) return;
    }
  }
}

function extractImages() {
  const candidateUrls = [];
  extractImagesFromNode(document, candidateUrls);
  return candidateUrls;
}

function extractPageData() {
  const title = document.title || "";
  const url = window.location.href;
  const html = document.documentElement ? document.documentElement.outerHTML.slice(0, 150000) : "";
  const hostname = window.location.hostname.toLowerCase();
  const isReddit = /reddit\.com/i.test(hostname);
  const isGoogle = /google\.[a-z.]+\/search/i.test(url);
  const runner = window.__WAE_ADAPTER_RUNNER__;

  let bodyText = "", domMethod = "fallback_text", pageType = "webpage";

  if (isReddit && runner) {
    const redditExtracted = runner.extractRedditContent();
    const fallbackText = document.body ? document.body.innerText.slice(0, 5000) : "";
    bodyText = redditExtracted.length > 30 ? redditExtracted : (redditExtracted ? redditExtracted + "\n" + fallbackText : fallbackText);
    domMethod = "adapter:reddit";
    pageType = "forum_thread";
  } else if (isGoogle && runner) {
    const googleExtracted = runner.extractGoogleSearchContent();
    const fallbackText = document.body ? document.body.innerText.slice(0, 5000) : "";
    bodyText = googleExtracted || fallbackText;
    domMethod = "adapter:google_search";
    pageType = "search_results";
  } else {
    bodyText = document.body ? document.body.innerText.slice(0, 5000) : "";
  }

  const chunkData = window.__WAE_CHUNK_SCANNER__ ? window.__WAE_CHUNK_SCANNER__.extract() : { candidates: [], totalExtracted: 0 };
  const images = extractImages();
  return {
    url, title, html, text: bodyText.slice(0, 6000), dom_method: domMethod,
    page_type: pageType, chunks: chunkData.candidates, total_extracted: chunkData.totalExtracted, images
  };
}

function isFeedReady() {
  const isGoogle = /google\.[a-z.]+\/search/i.test(window.location.href);
  if (isGoogle) {
    const results = document.querySelectorAll("#search .g, #rso .MjjYud, #rso > div");
    return results.length > 0 || ((document.body ? document.body.innerText.length : 0) > 100);
  }
  const isReddit = /reddit\.com/i.test(window.location.hostname);
  if (isReddit) {
    if (window.__WAE_ADAPTER_RUNNER__) return window.__WAE_ADAPTER_RUNNER__.isRedditReady();
    const words = (document.body ? document.body.innerText : "").trim().split(/\s+/).filter(Boolean);
    return words.length >= 25;
  }
  const isYouTube = /youtube\.com/i.test(window.location.hostname);
  if (isYouTube && window.location.pathname.startsWith("/watch")) {
    const player = document.querySelector("#movie_player, ytd-watch-flexy");
    const titleEl = document.querySelector("h1.ytd-watch-metadata, #title h1");
    if (player && titleEl && (titleEl.innerText || "").trim().length > 0) return true;
  }
  return true;
}

async function evaluateCurrentPage(isFinalAttempt = false, navId = currentNavigationId) {
  if (navId !== currentNavigationId) return;

  if (!isFinalAttempt && !isFeedReady()) {
    handleChallengePending(1000, navId);
    return;
  }

  const payload = extractPageData();
  try {
    const res = await chrome.runtime.sendMessage({
      type: "EVALUATE_PAGE",
      payload: Object.assign({}, payload, { force_evaluate: isFinalAttempt })
    });

    if (navId !== currentNavigationId) return;
    if (!res) {
      lockCurtain("daemon_unavailable");
      return;
    }

    shadowMode = res.shadow_mode === true;

    if (res.flagged_blocks && res.flagged_blocks.length > 0 && window.__WAE_CHUNK_SCANNER__) {
      window.__WAE_CHUNK_SCANNER__.redact(res.flagged_blocks, shadowMode);
    }

    if (res.verdict === "BLOCK") {
      console.warn(`[Leko Blocker] Page flagged as BLOCK (${res.policy_triggered}). Shadow mode=${shadowMode}`);
      if (!shadowMode) {
        lockCurtain(res.policy_triggered || "blocked");
        return;
      }
    }

    if (res.verdict === "PENDING") {
      if (isFinalAttempt) {
        if (window.__WAE_CURTAIN__) window.__WAE_CURTAIN__.lift();
        return;
      }
      handleChallengePending(res.retry_in_ms || 1500, navId);
      return;
    }

    if (window.__WAE_CURTAIN__) window.__WAE_CURTAIN__.lift();
    retryAttempts = 0;
    if (retryTimeout) clearTimeout(retryTimeout);
  } catch (err) {
    console.error("[Leko Blocker] Evaluation communication error (failing closed):", err);
    if (navId === currentNavigationId) lockCurtain("daemon_unavailable");
  }
}

function handleChallengePending(delayMs, navId) {
  if (retryAttempts >= MAX_RETRIES) {
    evaluateCurrentPage(true, navId);
    return;
  }
  retryAttempts++;
  if (retryTimeout) clearTimeout(retryTimeout);
  retryTimeout = setTimeout(() => {
    evaluateCurrentPage(false, navId);
  }, delayMs);
}

function waitForDOMStability(callback, quietMs = 400, maxWaitMs = 2000) {
  if (spaObserver) {
    spaObserver.disconnect();
    spaObserver = null;
  }
  if (spaStabilityTimeout) clearTimeout(spaStabilityTimeout);

  let timer = null;
  const startTime = Date.now();
  const settle = () => {
    if (spaObserver) spaObserver.disconnect();
    spaObserver = null;
    callback();
  };

  spaObserver = new MutationObserver(() => {
    if (Date.now() - startTime >= maxWaitMs) {
      settle();
      return;
    }
    clearTimeout(timer);
    timer = setTimeout(settle, quietMs);
  });

  const root = document.body || document.documentElement;
  if (root) spaObserver.observe(root, { childList: true, subtree: true });
  timer = setTimeout(settle, quietMs);
}

function scheduleEvaluation() {
  const thisNavId = currentNavigationId;
  waitForDOMStability(() => {
    evaluateCurrentPage(false, thisNavId);
  });
}

function triggerSpanNavigation(targetUrl) {
  const nextNorm = normalizeUrl(targetUrl || window.location.href);
  if (nextNorm === currentNormalizedUrl && !targetUrl) return;
  currentNormalizedUrl = nextNorm;
  currentNavigationId++;
  currentUrl = window.location.href;
  retryAttempts = 0;
  if (retryTimeout) clearTimeout(retryTimeout);
  if (window.__WAE_CURTAIN__) {
    window.__WAE_CURTAIN__.mount();
    window.__WAE_CURTAIN__.setStatus("Verifying updated route...");
  }
  scheduleEvaluation();
}

window.__WAE_ON_SPA_NAVIGATE__ = triggerSpanNavigation;

chrome.runtime.onMessage.addListener((msg) => {
  if (msg.type === "SPA_NAVIGATION" || msg.type === "TAB_URL_CHANGED") {
    if (msg.url && normalizeUrl(msg.url) !== currentNormalizedUrl) {
      triggerSpanNavigation(msg.url);
    }
  }
});

const originalPushState = history.pushState;
history.pushState = function (...args) {
  originalPushState.apply(this, args);
  if (normalizeUrl(window.location.href) !== currentNormalizedUrl) {
    triggerSpanNavigation();
  }
};

const originalReplaceState = history.replaceState;
history.replaceState = function (...args) {
  originalReplaceState.apply(this, args);
  if (normalizeUrl(window.location.href) !== currentNormalizedUrl) {
    triggerSpanNavigation();
  }
};

if (document.readyState === "complete" || document.readyState === "interactive") {
  scheduleEvaluation();
} else {
  window.addEventListener("DOMContentLoaded", scheduleEvaluation);
}
