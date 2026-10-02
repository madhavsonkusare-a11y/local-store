// Shared, opt-in content proof for isolated qualification installs.
// Usage: node content-roundtrip-probe.mjs first-use|verify URL STATE memos
//     or node content-roundtrip-probe.mjs first-use|verify URL STATE flatnotes USER PASSWORD
// Keep adapters in this file: ScriptProbe and the release gate fingerprint the
// exact script bytes, so changing an adapter invalidates retained evidence.
import { randomBytes } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const prefix = 'Local Store content proof ';
const [phase, address, statePath, adapterName, ...options] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !statePath || !['memos', 'flatnotes'].includes(adapterName)) {
  throw new Error('first-use|verify URL STATE memos|flatnotes [USER PASSWORD] required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('content proof requires an unadorned loopback HTTP address');
}

async function request(path, { method = 'GET', body, token } = {}) {
  const response = await fetch(new URL(path, base), {
    method,
    redirect: 'manual',
    signal: AbortSignal.timeout(15000),
    headers: {
      ...(body !== undefined ? { 'content-type': 'application/json' } : {}),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });
  if (!response.ok) throw new Error(`${method} ${path} returned ${response.status}`);
  const json = await response.json();
  if (!json || typeof json !== 'object' || Array.isArray(json)) {
    throw new Error(`${method} ${path} returned no JSON object`);
  }
  return json;
}

const adapters = {
  memos: {
    async setup(state) {
      const profile = await request('/api/v1/instance/profile');
      if (profile.needsSetup !== true) throw new Error('Memos was already set up');
      state.username = 'localstoreproof';
      state.password = randomBytes(24).toString('hex');
      const account = await request('/api/v1/users', {
        method: 'POST', body: { username: state.username, password: state.password, role: 'ADMIN', state: 'NORMAL' },
      });
      if (account.username !== state.username || account.role !== 'ADMIN') throw new Error('Memos admin setup failed');
    },
    async authenticate(state) {
      const profile = await request('/api/v1/instance/profile');
      if (profile.needsSetup !== false) throw new Error('Memos lost admin setup');
      const signIn = await request('/api/v1/auth/signin', {
        method: 'POST', body: { passwordCredentials: { username: state.username, password: state.password } },
      });
      if (typeof signIn.accessToken !== 'string' || signIn.accessToken.length < 16) {
        throw new Error('Memos did not issue an access token');
      }
      return signIn.accessToken;
    },
    async create(state, token) {
      const memo = await request('/api/v1/memos', {
        method: 'POST', token, body: { content: state.content, visibility: 'PRIVATE', state: 'NORMAL' },
      });
      if (!/^memos\/[a-zA-Z0-9_-]+$/.test(memo.name) || memo.content !== state.content || memo.visibility !== 'PRIVATE') {
        throw new Error('Memos did not create the exact private memo');
      }
      return memo.name;
    },
    async read(state, token) {
      if (!/^memos\/[a-zA-Z0-9_-]+$/.test(state.item)) throw new Error('invalid Memos item identity');
      const memo = await request(`/api/v1/${state.item}`, { token });
      if (memo.visibility !== 'PRIVATE') throw new Error('Memos content lost private visibility');
      return { item: memo.name, content: memo.content };
    },
  },
  flatnotes: {
    async setup(state) {
      if (options.length !== 2 || !options[0] || !options[1]) throw new Error('flatnotes requires USER PASSWORD');
      state.username = options[0];
      state.password = options[1];
    },
    async authenticate(state) {
      // A known-wrong password must fail, or a working token proves little.
      const wrong = await fetch(new URL('/api/token', base), {
        method: 'POST', redirect: 'manual', signal: AbortSignal.timeout(15000),
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ username: state.username, password: `${state.password}-wrong` }),
      });
      if (wrong.ok) throw new Error('flatnotes accepted a wrong password');
      const signIn = await request('/api/token', {
        method: 'POST', body: { username: state.username, password: state.password },
      });
      if (typeof signIn.access_token !== 'string' || !signIn.access_token) {
        throw new Error('flatnotes did not issue an access token');
      }
      return signIn.access_token;
    },
    async create(state, token) {
      const slug = `local-store-${randomBytes(8).toString('hex')}`;
      await request('/api/notes', { method: 'POST', token, body: { title: slug, content: state.content } });
      return slug;
    },
    async read(state, token) {
      if (!/^local-store-[a-f0-9]{16}$/.test(state.item)) throw new Error('invalid flatnotes item identity');
      const note = await request(`/api/notes/${state.item}`, { token });
      return { item: state.item, content: note.content };
    },
  },
};

let state;
if (phase === 'first-use') {
  state = { schema: 1, adapter: adapterName, content: `${prefix}${randomBytes(16).toString('hex')}` };
  await adapters[adapterName].setup(state);
} else {
  state = JSON.parse(await readFile(statePath, 'utf8'));
  if (state.schema !== 1 || state.adapter !== adapterName ||
      typeof state.content !== 'string' || !/^Local Store content proof [a-f0-9]{32}$/.test(state.content) ||
      typeof state.username !== 'string' || !state.username ||
      typeof state.password !== 'string' || !state.password || typeof state.item !== 'string') {
    throw new Error('content proof state is invalid');
  }
  if (adapterName === 'flatnotes' && options.length === 2 &&
      (options[0] !== state.username || options[1] !== state.password)) {
    throw new Error('flatnotes install answers changed');
  }
}
const token = await adapters[adapterName].authenticate(state);
if (phase === 'first-use') state.item = await adapters[adapterName].create(state, token);
const observed = await adapters[adapterName].read(state, token);
if (observed.item !== state.item || observed.content !== state.content) {
  throw new Error('exact content roundtrip failed');
}
if (phase === 'first-use') {
  // Persist only after an independent read; never overwrite a prior proof.
  await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
}
console.log(`${adapterName} content ${phase} passed`);
