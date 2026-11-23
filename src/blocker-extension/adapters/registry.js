// Append-only Declarative Site Adapter Registry
export class AdapterRegistry {
  constructor() {
    this.adapters = new Map();
  }

  async init() {
    // 1. Load built-ins
    for (const name of ['reddit', 'youtube', 'google_search']) {
      try {
        const url = chrome.runtime.getURL(`adapters/${name}.json`);
        const res = await fetch(url);
        if (res.ok) {
          const adapter = await res.json();
          this.adapters.set(adapter.id, adapter);
        }
      } catch (e) {
        console.warn(`Failed loading built-in adapter ${name}:`, e);
      }
    }

    // 2. Load custom / dynamic adapters from storage
    if (typeof chrome !== 'undefined' && chrome.storage && chrome.storage.local) {
      const stored = await chrome.storage.local.get('custom_adapters');
      if (stored && stored.custom_adapters) {
        for (const [id, adapter] of Object.entries(stored.custom_adapters)) {
          this.adapters.set(id, adapter);
        }
      }
    }
  }

  findAdapter(url) {
    for (const adapter of this.adapters.values()) {
      for (const pattern of adapter.url_patterns || []) {
        if (this.matchPattern(pattern, url)) {
          return adapter;
        }
      }
    }
    return null;
  }

  matchPattern(pattern, url) {
    const escaped = pattern
      .replace(/\*/g, '.*')
      .replace(/\?/g, '.');
    return new RegExp(`^${escaped}$`, 'i').test(url);
  }

  // Append-only update: merges new adapters or new selectors. Never deletes.
  async mergeRemoteManifest(rawManifestUrl) {
    const res = await fetch(rawManifestUrl);
    if (!res.ok) {
      throw new Error(`Failed to fetch remote adapter manifest: ${res.status}`);
    }
    const manifest = await res.json();
    if (!manifest.adapters || !Array.isArray(manifest.adapters)) {
      throw new Error('Invalid manifest structure: missing adapters array');
    }

    const custom = (await chrome.storage.local.get('custom_adapters')).custom_adapters || {};
    let addedCount = 0;

    for (const item of manifest.adapters) {
      if (!item.id || !item.url_patterns) continue;
      const existing = this.adapters.get(item.id) || custom[item.id];
      if (existing) {
        // Merge patterns and selectors without overwriting existing
        const patterns = Array.from(new Set([...(existing.url_patterns || []), ...item.url_patterns]));
        const selectors = { ...(existing.selectors || {}), ...(item.selectors || {}) };
        custom[item.id] = { ...existing, ...item, url_patterns: patterns, selectors };
      } else {
        custom[item.id] = item;
        addedCount++;
      }
      this.adapters.set(item.id, custom[item.id]);
    }

    await chrome.storage.local.set({ custom_adapters: custom });
    return { success: true, added: addedCount, total: this.adapters.size };
  }
}
