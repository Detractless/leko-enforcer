const API_ROOT = "https://api.github.com";

function encodeBase64Url(value) {
  const bytes = value instanceof Uint8Array ? value : new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
}

function pemBytes(pem) {
  const base64 = pem.replace(/-----BEGIN PRIVATE KEY-----|-----END PRIVATE KEY-----|\s/g, "");
  return Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
}

async function createAppJwt(env) {
  const now = Math.floor(Date.now() / 1000);
  const header = encodeBase64Url(JSON.stringify({ alg: "RS256", typ: "JWT" }));
  const payload = encodeBase64Url(JSON.stringify({ iat: now - 60, exp: now + 540, iss: env.GITHUB_APP_ID }));
  const unsigned = `${header}.${payload}`;
  const key = await crypto.subtle.importKey(
    "pkcs8",
    pemBytes(env.GITHUB_APP_PRIVATE_KEY),
    { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign("RSASSA-PKCS1-v1_5", key, new TextEncoder().encode(unsigned));
  return `${unsigned}.${encodeBase64Url(new Uint8Array(signature))}`;
}

async function githubRequest(url, options, fetchImpl) {
  return fetchImpl(url, {
    ...options,
    headers: {
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      ...options.headers,
    },
  });
}

export async function getInstallationToken(env, fetchImpl = fetch) {
  const jwt = await createAppJwt(env);
  const url = `${API_ROOT}/app/installations/${encodeURIComponent(env.GITHUB_INSTALLATION_ID)}/access_tokens`;
  const response = await githubRequest(url, {
    method: "POST",
    headers: { Authorization: `Bearer ${jwt}`, "Content-Type": "application/json" },
    body: JSON.stringify({ repositories: [env.GITHUB_REPOSITORY], permissions: { contents: "write" } }),
  }, fetchImpl);
  if (!response.ok) throw new Error(`GitHub token request failed (${response.status})`);
  const result = await response.json();
  if (!result.token) throw new Error("GitHub did not return an installation token");
  return result.token;
}

export async function writeCapture(env, record, deletionTokenHash, fetchImpl = fetch) {
  const token = await getInstallationToken(env, fetchImpl);
  const path = `incoming/${record.captureId}.json`;
  const url = `${API_ROOT}/repos/${encodeURIComponent(env.GITHUB_OWNER)}/${encodeURIComponent(env.GITHUB_REPOSITORY)}/contents/${path}`;
  const storedRecord = { ...record, deletionTokenHash };
  const serializedRecord = JSON.stringify(storedRecord, null, 2) + "\n";
  const content = encodeBase64UrlUtf8(serializedRecord);
  const put = await githubRequest(url, {
    method: "PUT",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ message: `Add incoming capture ${record.captureId}`, content, branch: env.GITHUB_BRANCH || "main" }),
  }, fetchImpl);
  if (put.ok) return { duplicate: false };
  if (![409, 422].includes(put.status)) throw new Error(`GitHub capture write failed (${put.status})`);

  const existing = await githubRequest(url, { headers: { Authorization: `Bearer ${token}` } }, fetchImpl);
  if (!existing.ok) throw new Error(`GitHub duplicate check failed (${existing.status})`);
  const saved = await existing.json();
  const savedContent = decodeGitHubContent(saved.content ?? "");
  if (savedContent !== serializedRecord) {
    throw new Error("captureId already exists with different data");
  }
  return { duplicate: true };
}

export async function deleteCapture(env, captureId, deletionTokenHash, fetchImpl = fetch) {
  const token = await getInstallationToken(env, fetchImpl);
  const path = `incoming/${captureId}.json`;
  const url = `${API_ROOT}/repos/${encodeURIComponent(env.GITHUB_OWNER)}/${encodeURIComponent(env.GITHUB_REPOSITORY)}/contents/${path}`;
  const headers = { Authorization: `Bearer ${token}` };
  const existing = await githubRequest(url, { headers }, fetchImpl);
  if (existing.status === 404) return { deleted: false };
  if (!existing.ok) throw new Error(`GitHub capture lookup failed (${existing.status})`);
  const saved = await existing.json();
  let record;
  try {
    record = JSON.parse(decodeGitHubContent(saved.content ?? ""));
  } catch {
    throw new Error("GitHub capture record is invalid");
  }
  if (record.deletionTokenHash !== deletionTokenHash) return { deleted: false };
  const removed = await githubRequest(url, {
    method: "DELETE",
    headers: { ...headers, "Content-Type": "application/json" },
    body: JSON.stringify({ message: `Delete incoming capture ${captureId}`, sha: saved.sha, branch: env.GITHUB_BRANCH || "main" }),
  }, fetchImpl);
  if (!removed.ok) throw new Error(`GitHub capture delete failed (${removed.status})`);
  return { deleted: true };
}

function encodeBase64UrlUtf8(value) {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function decodeGitHubContent(value) {
  const compact = value.replace(/\s/g, "");
  return new TextDecoder().decode(Uint8Array.from(atob(compact), (char) => char.charCodeAt(0)));
}
