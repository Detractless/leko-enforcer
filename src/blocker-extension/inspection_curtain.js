// inspection_curtain.js - Closed Shadow DOM Full-Viewport Overlay with SPA Navigation Lifecycle
// Mounts at document_start & in-app SPA transitions to prevent screen flash while underlying DOM hydrates.

(function () {
  let host = null;
  let shadow = null;
  let statusEl = null;
  let locked = false;

  function normalizeUrl(u) {
    try {
      const parsed = new URL(u, window.location.href);
      return parsed.origin + parsed.pathname + parsed.search;
    } catch (_) {
      return (u || '').split('#')[0];
    }
  }

  let lastEvaluatedNormalizedUrl = normalizeUrl(window.location.href);

  function mountCurtain() {
    const existing = document.getElementById('__wae_curtain_host__');
    if (existing && shadow) {
      const container = shadow.querySelector('.curtain-overlay');
      if (container) {
        container.classList.remove('curtain-hidden');
        if (statusEl) statusEl.textContent = 'Verifying page security...';
        locked = false;
        return;
      }
    }

    locked = false;
    host = document.createElement('div');
    host.id = '__wae_curtain_host__';
    host.style.cssText = 'all: initial !important; position: static !important;';

    shadow = host.attachShadow({ mode: 'closed' });

    const style = document.createElement('style');
    style.textContent = `
      :host {
        all: initial !important;
      }
      .curtain-overlay {
        position: fixed !important;
        top: 0 !important;
        left: 0 !important;
        width: 100vw !important;
        height: 100vh !important;
        background-color: #121212 !important;
        color: #e0e0e0 !important;
        z-index: 2147483647 !important;
        display: flex !important;
        flex-direction: column !important;
        align-items: center !important;
        justify-content: center !important;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif !important;
        transition: opacity 0.12s cubic-bezier(0, 0, 0.2, 1) !important;
        pointer-events: auto !important;
        box-sizing: border-box !important;
        user-select: none !important;
        border-radius: 0 !important;
      }
      .curtain-status {
        font-size: 13px !important;
        font-weight: 500 !important;
        letter-spacing: 0.02em !important;
        color: #9e9e9e !important;
      }
      .curtain-hidden {
        opacity: 0 !important;
        pointer-events: none !important;
      }
      .curtain-block-title {
        font-size: 22px !important;
        font-weight: 600 !important;
        color: #f5f5f5 !important;
        margin-bottom: 10px !important;
      }
      .curtain-block-detail {
        font-family: ui-monospace, SFMono-Regular, Menlo, monospace !important;
        font-size: 13px !important;
        color: #ff8a80 !important;
        margin-bottom: 14px !important;
      }
      .curtain-block-hint {
        font-size: 12px !important;
        color: #9e9e9e !important;
      }
    `;

    const container = document.createElement('div');
    container.className = 'curtain-overlay';

    statusEl = document.createElement('div');
    statusEl.className = 'curtain-status';
    statusEl.textContent = 'Verifying page security...';

    container.appendChild(statusEl);
    shadow.appendChild(style);
    shadow.appendChild(container);

    const rootTarget = document.documentElement || document.body;
    if (rootTarget) {
      rootTarget.appendChild(host);
    } else {
      const obs = new MutationObserver(() => {
        const t = document.documentElement || document.body;
        if (t) {
          t.appendChild(host);
          obs.disconnect();
        }
      });
      obs.observe(document, { childList: true });
    }
  }

  function setStatus(text) {
    if (statusEl) statusEl.textContent = text;
  }

  function showBlock(policy) {
    locked = true;
    if (!shadow) return;
    const container = shadow.querySelector('.curtain-overlay');
    if (!container) return;
    container.classList.remove('curtain-hidden');
    container.innerHTML = '';
    const unavailable = policy === 'daemon_unavailable';
    const title = document.createElement('div');
    title.className = 'curtain-block-title';
    title.textContent = unavailable ? 'Daemon unavailable' : 'Blocked by policy';
    const detail = document.createElement('div');
    detail.className = 'curtain-block-detail';
    detail.textContent = policy || 'blocked';
    const hint = document.createElement('div');
    hint.className = 'curtain-block-hint';
    hint.textContent = unavailable
      ? 'Page held safely until the local policy daemon is reachable.'
      : 'This page is not available under the active policy profile.';
    container.appendChild(title);
    container.appendChild(detail);
    container.appendChild(hint);
    statusEl = hint;
  }

  function liftCurtain() {
    if (locked) return;
    lastEvaluatedNormalizedUrl = normalizeUrl(window.location.href);
    if (!host || !shadow) return;
    const container = shadow.querySelector('.curtain-overlay');
    if (container) {
      container.classList.add('curtain-hidden');
      setTimeout(() => {
        if (host && host.parentNode) host.parentNode.removeChild(host);
        host = null;
        shadow = null;
        statusEl = null;
      }, 150);
    } else if (host.parentNode) {
      host.parentNode.removeChild(host);
      host = null;
      shadow = null;
      statusEl = null;
    }
  }

  window.__WAE_CURTAIN__ = {
    mount: mountCurtain,
    setStatus: setStatus,
    lift: liftCurtain,
    showBlock: showBlock,
    normalizeUrl: normalizeUrl
  };

  mountCurtain();

  // Multi-signal SPA Route Transition Detection (document_start level)
  window.addEventListener('yt-navigate-start', () => {
    mountCurtain();
    if (window.__WAE_ON_SPA_NAVIGATE__) window.__WAE_ON_SPA_NAVIGATE__();
  }, true);

  if (window.navigation && window.navigation.addEventListener) {
    window.navigation.addEventListener('navigate', (e) => {
      const destUrl = e.destination ? e.destination.url : '';
      if (!destUrl) return;
      const targetNormalized = normalizeUrl(destUrl);
      if (targetNormalized === lastEvaluatedNormalizedUrl) return; // Same-document fragment update
      if (e.canIntercept || e.userInitiated || e.navigationType !== 'reload') {
        lastEvaluatedNormalizedUrl = targetNormalized;
        mountCurtain();
        if (window.__WAE_ON_SPA_NAVIGATE__) window.__WAE_ON_SPA_NAVIGATE__(destUrl);
      }
    });
  }

  window.addEventListener('popstate', () => {
    const currentNorm = normalizeUrl(window.location.href);
    if (currentNorm === lastEvaluatedNormalizedUrl) return;
    lastEvaluatedNormalizedUrl = currentNorm;
    mountCurtain();
    if (window.__WAE_ON_SPA_NAVIGATE__) window.__WAE_ON_SPA_NAVIGATE__(window.location.href);
  }, true);
})();
