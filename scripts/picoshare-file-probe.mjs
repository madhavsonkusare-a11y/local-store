// Synthetic loopback-only qualification. Selectors follow PicoShare v1.5.4's
// official login/upload tests; this script never receives an owner's account.
import {chromium, expect} from '@playwright/test';
import {readFile, writeFile} from 'node:fs/promises';
import {randomBytes, createHash} from 'node:crypto';
const [phase,address,statePath] = process.argv.slice(2);
if (!['first-use','verify'].includes(phase) || !statePath) throw new Error('first-use|verify URL STATE required');
const base = new URL(address);
if (base.protocol !== 'http:' || !['127.0.0.1','localhost','[::1]'].includes(base.hostname) || base.username || base.password || base.pathname !== '/' || base.search || base.hash) throw new Error('isolated unadorned loopback HTTP required');
let state = JSON.parse(await readFile(statePath,'utf8'));
if (typeof state.password !== 'string' || state.password.length < 32) throw new Error('isolated proof passphrase missing');
const browser = await chromium.launch({headless:true,channel:'msedge'});
try {
  const page = await browser.newPage(); page.setDefaultTimeout(30000);
  async function login(password) {
    await page.goto(new URL('/login',base).href);
    await page.locator("form input[type='password']").fill(password);
    await page.locator("form input[type='submit']").click();
  }
  if (phase === 'first-use') {
    await login(randomBytes(24).toString('hex'));
    await expect(page.locator('.file-input')).toHaveCount(0);
  }
  await login(state.password);
  await expect(page.locator('.file-input')).toBeVisible();
  if (phase === 'first-use') {
    const payload = randomBytes(65536), filename = `local-store-proof-${randomBytes(8).toString('hex')}.bin`;
    const expiration = new Date(Date.now()+7*86400000).toISOString().slice(0,10);
    await page.locator('#expiration-select').selectOption({label:'Custom'});
    await page.locator('#expiration-picker #expiration').fill(expiration);
    await page.locator('#note').fill('Local Store isolated qualification file');
    await page.locator('.file-input').setInputFiles({name:filename,mimeType:'application/octet-stream',buffer:payload});
    await expect(page.locator('#upload-result .message-body')).toHaveText('Upload complete!');
    state = {...state,filename,expiration,sha256:createHash('sha256').update(payload).digest('hex')};
  }
  await page.getByRole('menuitem',{name:'Files',exact:true}).click();
  const row = page.getByRole('row').filter({hasText:state.filename});
  await expect(row).toHaveCount(1);
  await expect(row.getByRole('cell').nth(1)).toHaveText('Local Store isolated qualification file');
  await expect(row.getByRole('cell').nth(4)).toContainText(state.expiration);
  const href = await row.getByRole('link',{name:state.filename,exact:true}).getAttribute('href');
  const download = new URL(href,base);
  if (download.origin !== base.origin || download.username || download.password) throw new Error('download escaped the isolated app');
  // A separate anonymous client proves the share is useful without giving an
  // admin session to the reader. The chosen future expiration survives restart.
  const response = await fetch(download,{redirect:'manual',signal:AbortSignal.timeout(15000)});
  if (!response.ok || Number(response.headers.get('content-length')) > 1048576) throw new Error('bounded anonymous share download failed');
  const chunks = [], reader = response.body.getReader(); let size = 0;
  for (;;) {
    const {value,done} = await reader.read(); if (done) break;
    size += value.length;
    if (size > 1048576) { await reader.cancel(); throw new Error('share response exceeded the byte limit'); }
    chunks.push(Buffer.from(value));
  }
  const bytes = Buffer.concat(chunks);
  if (bytes.length !== 65536 || createHash('sha256').update(bytes).digest('hex') !== state.sha256) throw new Error('shared file bytes changed');
  if (phase === 'first-use') await writeFile(statePath,JSON.stringify(state),{mode:0o600});
  console.log('PicoShare exact file, expiry and anonymous read passed');
} finally { await browser.close(); }
