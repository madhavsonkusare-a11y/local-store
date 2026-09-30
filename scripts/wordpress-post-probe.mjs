// Opt-in managed-engine proof for WordPress 7.1.0 and MariaDB 11.4.13.
// Usage: node wordpress-post-probe.mjs first-use|verify LOOPBACK_URL STATE
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
  throw new Error('WordPress proof requires an unadorned loopback HTTP address');
}
const apiBase = new URL(base);
apiBase.hostname = '127.0.0.1';
const state = phase === 'first-use' ? {
  schema: 1,
  username: `localstore${randomBytes(5).toString('hex')}`,
  password: `${randomBytes(18).toString('base64url')}Aa1!`,
  email: `local-store-${randomBytes(8).toString('hex')}@example.invalid`,
  siteTitle: `Local Store proof ${randomUUID()}`,
  postTitle: `Launch post ${randomUUID()}`,
  content: `Exact WordPress content ${randomUUID()}`,
} : JSON.parse(await readFile(statePath, 'utf8'));
if (phase === 'verify' && (state.schema !== 1 ||
    !/^localstore[a-f0-9]{10}$/.test(state.username) ||
    !/^Launch post [a-f0-9-]{36}$/.test(state.postTitle) ||
    !/^Exact WordPress content [a-f0-9-]{36}$/.test(state.content) ||
    !/^[1-9][0-9]*$/.test(state.postId))) {
  throw new Error('WordPress proof state is invalid');
}

const browser = await chromium.launch({ headless: true, channel: 'msedge' });
try {
  const page = await browser.newPage();
  page.setDefaultTimeout(30000);
  await page.goto(base.href, { waitUntil: 'domcontentloaded' });
  if (phase === 'first-use') {
    if (!page.url().includes('/wp-admin/install.php')) {
      throw new Error(`WordPress did not show its installer at ${page.url()}`);
    }
    let stage = 'language';
    try {
    await page.locator('#language-continue').click();
    stage = 'site title';
    await page.locator('#weblog_title').fill(state.siteTitle);
    stage = 'administrator name';
    await page.locator('#user_login').fill(state.username);
    stage = 'administrator password';
    await page.locator('#pass1').fill(state.password);
    // The JS-enabled installer hides the repeat field, while PHP still requires it.
    stage = 'administrator password confirmation';
    await page.locator('#pass2').evaluate((input, value) => {
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }, state.password);
    stage = 'administrator email';
    await page.locator('#admin_email').fill(state.email);
    stage = 'install submission';
    await page.locator('#submit').click();
    stage = 'install success';
    await page.getByText('Success!', { exact: true }).waitFor({ timeout: 60000 });
    } catch {
      const body = await page.locator('body').innerText({ timeout: 2000 }).catch(() => '<body unavailable>');
      throw new Error(`WordPress setup failed at ${stage}, ${page.url()}: ${body.slice(-500)}`);
    }
  } else if (page.url().includes('/wp-admin/install.php')) {
    throw new Error('WordPress lost its installed site after restart or reinstall');
  }

  function xmlText(value) {
    return String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&apos;');
  }
  function xmlString(value) { return `<value><string>${xmlText(value)}</string></value>`; }
  async function xmlrpc(method, params) {
    const request = `<?xml version="1.0"?><methodCall><methodName>${method}</methodName><params>${params.map(value => `<param>${value}</param>`).join('')}</params></methodCall>`;
    const response = await fetch(new URL('/xmlrpc.php', apiBase), {
      method: 'POST', headers: { 'content-type': 'text/xml' }, body: request,
    });
    const text = await response.text();
    if (!response.ok || text.includes('<fault>')) {
      throw new Error(`WordPress ${method} returned HTTP ${response.status}: ${text.slice(0, 250)}`);
    }
    return text;
  }

  if (phase === 'first-use') {
    const content = `<value><struct><member><name>post_type</name>${xmlString('post')}</member><member><name>post_status</name>${xmlString('publish')}</member><member><name>post_title</name>${xmlString(state.postTitle)}</member><member><name>post_content</name>${xmlString(state.content)}</member></struct></value>`;
    const result = await xmlrpc('wp.newPost', [
      '<value><int>1</int></value>', xmlString(state.username),
      xmlString(state.password), content,
    ]);
    state.postId = result.match(/<value>\s*<(?:string|int|i4)>([1-9][0-9]*)<\/(?:string|int|i4)>\s*<\/value>/)?.[1];
    if (!state.postId) throw new Error('WordPress did not return a post identity');
  }

  const response = await fetch(new URL(`/?rest_route=/wp/v2/posts/${state.postId}`, apiBase));
  if (!response.ok) throw new Error(`WordPress published post returned HTTP ${response.status}`);
  const post = await response.json();
  const exactText = String(post.content?.rendered ?? '').replace(/<[^>]*>/g, '').trim();
  if (String(post.id) !== state.postId || post.status !== 'publish' ||
      post.title?.rendered !== state.postTitle || exactText !== state.content) {
    throw new Error('WordPress did not retain the exact published post');
  }

  if (phase === 'first-use') {
    await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
  }
  console.log(`WordPress exact published post ${phase} passed`);
} finally {
  await browser.close();
}
