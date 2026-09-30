// Opt-in Immich v3.2 managed-engine proof: admin signup, exact photo search/download.
// Usage: node immich-photo-probe.mjs first-use|verify LOOPBACK_URL STATE
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [phase, address, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !address || !statePath) {
  throw new Error('first-use|verify LOOPBACK_URL STATE required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('Immich proof requires an unadorned loopback HTTP address');
}
base.hostname = '127.0.0.1';
const photo = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAE0lEQVR4nGP8lK3NAANMcBZeDgBOWwGQrEUIaQAAAABJRU5ErkJggg==', 'base64');
const photoHash = createHash('sha256').update(photo).digest('hex');
const state = phase === 'first-use' ? {
  schema: 1,
  email: `local-store-${randomBytes(5).toString('hex')}@example.invalid`,
  password: `${randomBytes(18).toString('base64url')}Aa1!`,
  filename: `Local Store Proof ${randomUUID()}.png`,
  photoHash,
} : JSON.parse(await readFile(statePath, 'utf8'));
if (phase === 'verify' && (state.schema !== 1 ||
    !/^local-store-[a-f0-9]{10}@example\.invalid$/.test(state.email) ||
    !/^Local Store Proof [a-f0-9-]{36}\.png$/.test(state.filename) ||
    !/^[a-f0-9-]{36}$/.test(state.assetId) || state.photoHash !== photoHash)) {
  throw new Error('Immich proof state is invalid');
}

let token;
async function api(path, method = 'GET', body) {
  const multipart = body instanceof FormData;
  const response = await fetch(new URL(`/api${path}`, base), {
    method,
    headers: {
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(body && !multipart ? { 'content-type': 'application/json' } : {}),
    },
    ...(body ? { body: multipart ? body : JSON.stringify(body) } : {}),
    signal: AbortSignal.timeout(15000),
  });
  if (!response.ok) {
    throw new Error(`Immich ${method} ${path} returned HTTP ${response.status}: ${(await response.text()).slice(0, 220)}`);
  }
  if (response.status === 204) return response;
  const type = response.headers.get('content-type') ?? '';
  return type.includes('application/json') ? response.json() : response;
}

if (phase === 'first-use') {
  await api('/auth/admin-sign-up', 'POST', {
    email: state.email, name: 'Local Store Proof', password: state.password,
  });
}
const login = await api('/auth/login', 'POST', { email: state.email, password: state.password });
if (!login.accessToken || login.isAdmin !== true || login.userEmail !== state.email) {
  throw new Error('Immich did not authenticate the first administrator');
}
token = login.accessToken;

if (phase === 'first-use') {
  const upload = new FormData();
  upload.append('assetData', new Blob([photo], { type: 'image/png' }), state.filename);
  upload.append('fileCreatedAt', '2026-09-30T00:00:00.000Z');
  upload.append('fileModifiedAt', '2026-09-30T00:00:00.000Z');
  const created = await api('/assets', 'POST', upload);
  if (!/^[a-f0-9-]{36}$/.test(created.id)) throw new Error('Immich did not return a photo ID');
  state.assetId = created.id;
}

let asset;
let found;
const deadline = Date.now() + 90000;
while (Date.now() < deadline) {
  asset = await api(`/assets/${state.assetId}`);
  const search = await api('/search/metadata', 'POST', { originalFileName: state.filename, size: 20 });
  found = search.assets?.items?.some(item => item.id === state.assetId);
  if (asset.originalFileName === state.filename && asset.width === 4 && asset.height === 4 && found) break;
  await new Promise(resolve => setTimeout(resolve, 500));
}
if (asset?.originalFileName !== state.filename || asset.width !== 4 || asset.height !== 4 || !found) {
  throw new Error('Immich did not process and find the exact uploaded photo');
}
const response = await api(`/assets/${state.assetId}/original`);
const downloaded = Buffer.from(await response.arrayBuffer());
if (createHash('sha256').update(downloaded).digest('hex') !== photoHash) {
  throw new Error('Immich did not return the original photo bytes');
}
if (phase === 'first-use') {
  await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
}
console.log(`Immich exact photo upload, search and download ${phase} passed`);
