(() => {
  if (window.top !== window.self || !document.body) return;
  let captureSent = false;

  function cleanUrl(value) {
    const url = new URL(value);
    url.username = "";
    url.password = "";
    url.hash = "";
    for (const key of [...url.searchParams.keys()]) {
      if (/^(utm_|fbclid$|gclid$|dclid$|msclkid$|mc_cid$|mc_eid$)/i.test(key)
        || /(token|auth|session|password|secret|signature|^sig$|^code$|^state$)/i.test(key)) {
        url.searchParams.delete(key);
      }
    }
    return url.toString();
  }

  async function captureIfEnabled() {
    if (captureSent) return;
    const settings = await chrome.storage.local.get(["collectionEnabled", "excludedHosts"]);
    if (settings.collectionEnabled !== true) return;
    const hostname = window.location.hostname.toLowerCase();
    const excluded = (settings.excludedHosts || []).includes(hostname);
    if (excluded) return;
    const text = (document.body.innerText || "").trim().slice(0, 100_000);
    if (!text) return;
    captureSent = true;
    const deletionToken = [...crypto.getRandomValues(new Uint8Array(32))]
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("");
    chrome.runtime.sendMessage({
      type: "queue-capture",
      capture: {
        captureId: crypto.randomUUID(),
        deletionToken,
        sourceUrl: cleanUrl(window.location.href),
        text,
        capturedAt: new Date().toISOString(),
        extractorVersion: "dom-readable-text-v1",
        consentVersion: "collection-opt-in-v1",
      },
    });
  }

  chrome.runtime.onMessage.addListener((message) => {
    if (message?.type === "capture-current-page") void captureIfEnabled();
  });
  void captureIfEnabled();
})();
