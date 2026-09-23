// Memos 0.30.0: create the first admin, write a private memo, and read that
// exact memo after restart and keep-data reinstall. Run only on an isolated
// qualification instance; this script creates real app content.
import { randomBytes } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [phase, address, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !statePath) {
  throw new Error('first-use|verify URL STATE required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('Memos proof requires an unadorned loopback HTTP address');
}

async function request(path, { method = 'GET', body, token } = {}) {
  const response = await fetch(new URL(path, base), {
    method,
    redirect: 'manual',
    signal: AbortSignal.timeout(15000),
    headers: {
      ...(body ? { 'content-type': 'application/json' } : {}),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  if (!response.ok) throw new Error(`${method} ${path} returned ${response.status}`);
  const json = await response.json();
  if (!json || typeof json !== 'object') throw new Error(`${path} returned no JSON object`);
  return json;
}

const username = 'localstoreproof';
let state;
if (phase === 'first-use') {
  const profile = await request('/api/v1/instance/profile');
  if (profile.needsSetup !== true) throw new Error('isolated Memos instance was already set up');
  const password = randomBytes(24).toString('hex');
  const created = await request('/api/v1/users', {
    method: 'POST',
    body: { username, password, role: 'ADMIN', state: 'NORMAL' },
  });
  if (created.username !== username || created.role !== 'ADMIN') {
    throw new Error('first-admin setup did not create the expected account');
  }
  state = {
    username,
    password,
    content: `Local Store managed-engine memo proof ${randomBytes(16).toString('hex')}`,
  };
} else {
  state = JSON.parse(await readFile(statePath, 'utf8'));
  if (state.username !== username || !/^memos\/[a-zA-Z0-9_-]+$/.test(state.memoName) ||
      typeof state.password !== 'string' || state.password.length < 32 ||
      typeof state.content !== 'string' || !state.content.startsWith('Local Store managed-engine memo proof ')) {
    throw new Error('Memos proof state is invalid');
  }
  const profile = await request('/api/v1/instance/profile');
  if (profile.needsSetup !== false) throw new Error('Memos lost its first-admin setup');
}

const signedIn = await request('/api/v1/auth/signin', {
  method: 'POST',
  body: { passwordCredentials: { username: state.username, password: state.password } },
});
if (typeof signedIn.accessToken !== 'string' || signedIn.accessToken.length < 16) {
  throw new Error('Memos did not issue an access token');
}
if (phase === 'first-use') {
  const memo = await request('/api/v1/memos', {
    method: 'POST',
    token: signedIn.accessToken,
    body: { content: state.content, visibility: 'PRIVATE', state: 'NORMAL' },
  });
  if (!/^memos\/[a-zA-Z0-9_-]+$/.test(memo.name) || memo.content !== state.content) {
    throw new Error('Memos did not create the requested private memo');
  }
  state.memoName = memo.name;
  await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
}
const memo = await request(`/api/v1/${state.memoName}`, { token: signedIn.accessToken });
if (memo.name !== state.memoName || memo.content !== state.content || memo.visibility !== 'PRIVATE') {
  throw new Error('the private Memos content did not survive this phase');
}
console.log(`Memos private memo ${phase} passed`);
