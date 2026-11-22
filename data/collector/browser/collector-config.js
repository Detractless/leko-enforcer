export const COLLECTOR_ENDPOINT = "https://REPLACE_WITH_YOUR_WORKER.workers.dev/v1/captures";
export const EXTRACTOR_VERSION = "dom-readable-text-v1";
export const CONSENT_VERSION = "collection-opt-in-v1";
export const MAX_CAPTURE_TEXT = 100_000;

export function isCollectorConfigured(endpoint = COLLECTOR_ENDPOINT) {
  return endpoint.startsWith("https://") && !endpoint.includes("REPLACE_WITH_YOUR_WORKER");
}
