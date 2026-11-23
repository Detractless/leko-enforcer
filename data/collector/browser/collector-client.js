import { isCollectorConfigured } from "./collector-config.js";

export async function submitCapture(capture, endpoint, fetchImpl = fetch) {
  if (!isCollectorConfigured(endpoint)) throw new Error("collector endpoint is not configured");
  const { deletionToken, ...record } = capture;
  const response = await fetchImpl(endpoint, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Idempotency-Key": record.captureId,
      "X-Deletion-Token": deletionToken,
    },
    body: JSON.stringify(record),
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`collector returned ${response.status}`);
  return response.json();
}

export async function deleteSubmittedCapture(captureId, deletionToken, endpoint, fetchImpl = fetch) {
  if (!isCollectorConfigured(endpoint)) throw new Error("collector endpoint is not configured");
  const response = await fetchImpl(`${endpoint}/${encodeURIComponent(captureId)}`, {
    method: "DELETE",
    headers: { "X-Deletion-Token": deletionToken },
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok && response.status !== 404) throw new Error(`collector returned ${response.status}`);
}
