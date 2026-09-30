// Opt-in managed-engine proof for Jellyfin 12.0 with owned sample audio.
// Usage: node jellyfin-media-probe.mjs first-use|verify LOOPBACK_URL STATE MEDIA_FILE
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [phase, address, statePath, mediaPath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !address || !statePath || !mediaPath) {
  throw new Error('first-use|verify LOOPBACK_URL STATE MEDIA_FILE required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('Jellyfin proof requires an unadorned loopback HTTP address');
}
base.hostname = '127.0.0.1';
const fixture = await readFile(mediaPath);
const fixtureHash = createHash('sha256').update(fixture).digest('hex');
const state = phase === 'first-use' ? {
  schema: 1,
  username: `localstore${randomBytes(5).toString('hex')}`,
  password: `${randomBytes(18).toString('base64url')}Aa1!`,
  library: `Local Store Media ${randomUUID()}`,
  filename: 'Local Store Proof Tone.wav',
  fixtureHash,
} : JSON.parse(await readFile(statePath, 'utf8'));
if (phase === 'verify' && (state.schema !== 1 ||
    !/^localstore[a-f0-9]{10}$/.test(state.username) ||
    !/^Local Store Media [a-f0-9-]{36}$/.test(state.library) ||
    state.filename !== 'Local Store Proof Tone.wav' ||
    state.fixtureHash !== fixtureHash ||
    !/^[a-f0-9-]{32,36}$/.test(state.itemId))) {
  throw new Error('Jellyfin proof state is invalid');
}

const deviceId = `local-store-proof-${randomUUID()}`;
let token;
async function api(path, method = 'GET', body) {
  const response = await fetch(new URL(path, base), {
    method,
    headers: {
      Authorization: `MediaBrowser Client="Local Store Proof", Device="Windows Test", DeviceId="${deviceId}", Version="1.0", Token="${token ?? ''}"`,
      ...(body ? { 'content-type': 'application/json' } : {}),
    },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  if (!response.ok) {
    throw new Error(`Jellyfin ${method} ${path} returned HTTP ${response.status}: ${(await response.text()).slice(0, 220)}`);
  }
  if (response.status === 204) return response;
  const type = response.headers.get('content-type') ?? '';
  return type.includes('application/json') ? response.json() : response;
}

async function publicInfoWhenReady() {
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    try { return await api('/System/Info/Public'); }
    catch (error) {
      if (!error.message.includes('returned HTTP 503')) throw error;
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  }
  throw new Error('Jellyfin did not finish loading in time');
}

if (phase === 'first-use') {
  const publicInfo = await publicInfoWhenReady();
  if (publicInfo.StartupWizardCompleted !== false) throw new Error('Jellyfin skipped its first-use wizard');
  // Jellyfin creates the first administrator when the wizard reads this endpoint.
  await api('/Startup/User');
  await api('/Startup/Configuration', 'POST', {
    ServerName: state.library, UICulture: 'en-US',
    MetadataCountryCode: 'US', PreferredMetadataLanguage: 'en',
  });
  let userUpdated = false;
  let userError;
  const userDeadline = Date.now() + 30000;
  while (Date.now() < userDeadline) {
    try {
      await api('/Startup/User', 'POST', { Name: state.username, Password: state.password });
      userUpdated = true;
      break;
    } catch (error) {
      userError = error;
      if (!/returned HTTP (404|503)/.test(error.message)) throw error;
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  }
  if (!userUpdated) {
    const wizard = await publicInfoWhenReady();
    throw new Error(`Jellyfin could not set first user; wizard=${wizard.StartupWizardCompleted}, ${userError?.message}`);
  }
  await api('/Startup/RemoteAccess', 'POST', { EnableRemoteAccess: false });
  await api('/Startup/Complete', 'POST');
} else {
  const publicInfo = await publicInfoWhenReady();
  if (publicInfo.StartupWizardCompleted !== true) throw new Error('Jellyfin lost its completed setup');
}

const authenticated = await api('/Users/AuthenticateByName', 'POST', {
  Username: state.username, Pw: state.password,
});
if (!authenticated.AccessToken || authenticated.User?.Name !== state.username) {
  throw new Error('Jellyfin did not authenticate its first administrator');
}
token = authenticated.AccessToken;

if (phase === 'first-use') {
  await api(`/Library/VirtualFolders?name=${encodeURIComponent(state.library)}&collectionType=music&paths=${encodeURIComponent('/media/data')}&refreshLibrary=true`, 'POST', {
    LibraryOptions: {},
  });
  await api('/Library/Refresh', 'POST');
}
const folders = await api('/Library/VirtualFolders');
if (!Array.isArray(folders) || !folders.some(folder =>
    folder.Name === state.library && folder.Locations?.includes('/media/data'))) {
  throw new Error('Jellyfin did not retain the selected media library');
}

let item;
const deadline = Date.now() + 60000;
while (Date.now() < deadline) {
  const result = await api(`/Items?Recursive=true&IncludeItemTypes=Audio&SearchTerm=${encodeURIComponent('Local Store Proof Tone')}&Limit=20`);
  item = result.Items?.find(candidate => candidate.Name === 'Local Store Proof Tone');
  if (item) break;
  await new Promise(resolve => setTimeout(resolve, 500));
}
if (!item?.Id) throw new Error('Jellyfin did not import the owned sample audio');
if (phase === 'verify' && item.Id !== state.itemId) {
  throw new Error('Jellyfin changed the sample media identity after restart or reinstall');
}
const stream = await api(`/Audio/${item.Id}/stream?static=true`);
const streamed = Buffer.from(await stream.arrayBuffer());
if (createHash('sha256').update(streamed).digest('hex') !== fixtureHash) {
  throw new Error('Jellyfin playback did not return the exact sample audio bytes');
}
if (phase === 'first-use') {
  state.itemId = item.Id;
  await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
}
console.log(`Jellyfin exact sample import and playback ${phase} passed`);
