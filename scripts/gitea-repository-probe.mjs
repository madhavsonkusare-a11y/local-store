// Opt-in managed-engine proof for the pinned Gitea 1.27.3 template.
// Usage: node gitea-repository-probe.mjs first-use|verify LOOPBACK_URL STATE
import { chromium } from '@playwright/test';
import { randomBytes, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [phase, address, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !address || !statePath) {
  throw new Error('first-use|verify LOOPBACK_URL STATE required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('Gitea proof requires an unadorned loopback HTTP address');
}

const state = phase === 'first-use' ? {
  schema: 1,
  username: `localstore${randomBytes(5).toString('hex')}`,
  email: `local-store-${randomBytes(8).toString('hex')}@example.invalid`,
  password: `${randomBytes(18).toString('base64url')}Aa1!`,
  repository: `proof-${randomBytes(6).toString('hex')}`,
  path: 'proof/installed.txt',
  content: `Local Store Gitea proof ${randomUUID()}\n`,
} : JSON.parse(await readFile(statePath, 'utf8'));
if (phase === 'verify' && (state.schema !== 1 ||
    !/^localstore[a-f0-9]{10}$/.test(state.username) ||
    !/^proof-[a-f0-9]{12}$/.test(state.repository) ||
    state.path !== 'proof/installed.txt' ||
    !/^Local Store Gitea proof [a-f0-9-]{36}\n$/.test(state.content) ||
    !/^[a-f0-9]{40}$/.test(state.commit))) {
  throw new Error('Gitea proof state is invalid');
}

const browser = await chromium.launch({ headless: true, channel: 'msedge' });
try {
  const page = await browser.newPage();
  page.setDefaultTimeout(30000);
  await page.goto(base.href, { waitUntil: 'domcontentloaded' });
  if (phase === 'first-use') {
    let stage = 'database host';
    try {
    await page.locator('#db_host').fill('gitea-db:5432');
    stage = 'database user';
    await page.locator('#db_user').fill('gitea');
    stage = 'database password';
    await page.locator('#db_passwd').fill('gitea');
    stage = 'database name';
    await page.locator('#db_name').fill('gitea');
    stage = 'application URL';
    await page.locator('#app_url').fill(base.href);
    await page.getByText('Administrator Account Settings', { exact: true }).click();
    stage = 'administrator name';
    await page.locator('#admin_name').fill(state.username);
    stage = 'administrator email';
    await page.locator('#admin_email').fill(state.email);
    stage = 'administrator password';
    await page.locator('#admin_passwd').fill(state.password);
    stage = 'administrator password confirmation';
    await page.locator('#admin_confirm_passwd').fill(state.password);
    stage = 'install submission';
      await page.getByRole('button', { name: 'Install Gitea' }).click();
    stage = 'install redirect';
      await page.waitForURL(url => !url.pathname.startsWith('/install'), { timeout: 15000 });
    } catch {
      const body = await page.locator('body').innerText({ timeout: 2000 }).catch(() => '<body unavailable>');
      throw new Error(`Gitea setup failed at ${stage}, ${page.url()}: ${body.slice(-700)}`);
    }
  } else if (new URL(page.url()).pathname.startsWith('/install')) {
    throw new Error('Gitea lost its installed configuration after restart or reinstall');
  }

  const authorization = `Basic ${Buffer.from(`${state.username}:${state.password}`).toString('base64')}`;
  const apiBase = new URL(base);
  apiBase.hostname = '127.0.0.1';
  async function api(path, method = 'GET', data) {
    const response = await page.request.fetch(new URL(path, apiBase).href, {
      method,
      headers: { authorization, ...(data ? { 'content-type': 'application/json' } : {}) },
      ...(data ? { data } : {}),
    });
    const text = await response.text();
    let body;
    try { body = JSON.parse(text); } catch { body = undefined; }
    if (!response.ok()) {
      throw new Error(`Gitea ${method} ${path} returned HTTP ${response.status()}: ${text.slice(0, 300)}`);
    }
    return body;
  }

  // The installer redirects before every HTTP handler has finished initializing.
  let user;
  let lastAuthError;
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    try { user = await api('/api/v1/user'); break; }
    catch (error) { lastAuthError = error; await page.waitForTimeout(500); }
  }
  if (user?.login !== state.username) {
    throw new Error(`Gitea did not authenticate the installed administrator: ${lastAuthError?.message ?? 'unexpected user response'}`);
  }

  const repoPath = `/api/v1/repos/${state.username}/${state.repository}`;
  if (phase === 'first-use') {
    const created = await api('/api/v1/user/repos', 'POST', {
      name: state.repository, private: true, auto_init: true,
    });
    if (created.name !== state.repository || created.private !== true || created.owner?.login !== state.username) {
      throw new Error('Gitea did not create the private repository');
    }
    const file = await api(`${repoPath}/contents/${state.path}`, 'POST', {
      content: Buffer.from(state.content).toString('base64'),
      message: 'Add exact Local Store proof file',
    });
    state.commit = file.commit?.sha;
    if (!/^[a-f0-9]{40}$/.test(state.commit)) throw new Error('Gitea did not return a commit identity');
  }

  const repository = await api(repoPath);
  if (repository.name !== state.repository || repository.private !== true ||
      repository.owner?.login !== state.username) {
    throw new Error('Gitea did not retain the private repository');
  }
  const file = await api(`${repoPath}/contents/${state.path}`);
  if (file.path !== state.path || Buffer.from(file.content ?? '', 'base64').toString() !== state.content) {
    throw new Error('Gitea did not retain the exact committed file');
  }
  const commit = await api(`${repoPath}/git/commits/${state.commit}`);
  if (commit.sha !== state.commit) throw new Error('Gitea did not retain the exact commit');

  if (phase === 'first-use') {
    await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
  }
  console.log(`Gitea private repository, exact file and commit ${phase} passed`);
} finally {
  await browser.close();
}
