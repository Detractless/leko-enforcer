const TRACKING_KEYS = new Set([
  "fbclid",
  "gclid",
  "dclid",
  "msclkid",
  "mc_cid",
  "mc_eid",
]);
const PRIVATE_KEYS = /(?:token|auth|session|password|secret|signature|^sig$|^code$|^state$)/i;
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const MAX_TEXT_LENGTH = 100_000;

export function cleanSourceUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new TypeError("sourceUrl must be a valid URL");
  }
  if (!["http:", "https:"].includes(url.protocol)) {
    throw new TypeError("sourceUrl must use HTTP or HTTPS");
  }
  url.username = "";
  url.password = "";
  for (const key of [...url.searchParams.keys()]) {
    if (key.toLowerCase().startsWith("utm_") || TRACKING_KEYS.has(key.toLowerCase()) || PRIVATE_KEYS.test(key)) {
      url.searchParams.delete(key);
    }
  }
  url.hash = "";
  return url.toString();
}

export function validateCapture(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError("record must be an object");
  }
  if (!UUID_PATTERN.test(input.captureId ?? "")) {
    throw new TypeError("captureId must be a UUID v4");
  }
  if (typeof input.text !== "string" || !input.text.trim() || input.text.length > MAX_TEXT_LENGTH) {
    throw new TypeError("text must contain 1 to 100000 characters");
  }
  if (typeof input.capturedAt !== "string" || !Number.isFinite(Date.parse(input.capturedAt))) {
    throw new TypeError("capturedAt must be an ISO date string");
  }
  if (typeof input.extractorVersion !== "string" || !input.extractorVersion.trim() || input.extractorVersion.length > 80) {
    throw new TypeError("extractorVersion is required");
  }
  if (typeof input.consentVersion !== "string" || !input.consentVersion.trim() || input.consentVersion.length > 80) {
    throw new TypeError("consentVersion is required");
  }
  const sourceUrl = cleanSourceUrl(input.sourceUrl);
  let adapter;
  if (input.adapter != null) {
    if (typeof input.adapter.id !== "string" || !input.adapter.id.trim()
      || typeof input.adapter.version !== "string" || !input.adapter.version.trim()) {
      throw new TypeError("adapter must include id and version");
    }
    adapter = { id: input.adapter.id.slice(0, 120), version: input.adapter.version.slice(0, 80) };
  }
  return {
    captureId: input.captureId.toLowerCase(),
    sourceUrl,
    text: input.text,
    capturedAt: new Date(input.capturedAt).toISOString(),
    extractorVersion: input.extractorVersion,
    ...(adapter ? { adapter } : {}),
    consentVersion: input.consentVersion,
  };
}
