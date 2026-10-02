// Uses Linkding v1.47.0's official session and bookmark forms. No owner's
// credentials are read and the browser visits only the isolated loopback app.
import {chromium, expect} from '@playwright/test';
import {readFile, writeFile} from 'node:fs/promises';
import {randomBytes} from 'node:crypto';
const [phase,address,statePath] = process.argv.slice(2);
if (!['first-use','verify'].includes(phase) || !statePath) throw new Error('first-use|verify URL STATE required');
const base = new URL(address);
if (base.protocol !== 'http:' || !['127.0.0.1','localhost','[::1]'].includes(base.hostname) || base.username || base.password || base.pathname !== '/' || base.search || base.hash) throw new Error('isolated unadorned loopback HTTP required');
let state = JSON.parse(await readFile(statePath,'utf8'));
if (typeof state.password !== 'string' || state.password.length < 32 || !/^[a-z0-9-]+$/.test(state.username)) throw new Error('isolated proof credentials missing');
const browser = await chromium.launch({headless:true,channel:'msedge'});
try {
  const context = await browser.newContext();
  const page = await context.newPage(); page.setDefaultTimeout(30000);
  async function login(password) {
    await page.goto(new URL('/login/',base).href);
    await page.locator('#id_username').fill(state.username);
    await page.locator('#id_password').fill(password);
    const csrf = await page.locator('form input[name=csrfmiddlewaretoken]').inputValue();
    // Official login tests require 401 for rejection and 302 for success.
    // Submit the session-bound form directly to avoid Turbo's asynchronous
    // error-page rendering racing the next protected-page navigation.
    return context.request.post(new URL('/login/',base).href,{form:{csrfmiddlewaretoken:csrf,username:state.username,password,next:'/bookmarks'},headers:{Referer:page.url()},maxRedirects:0});
  }
  if (phase === 'first-use') {
    const denied = await context.request.get(new URL('/bookmarks',base).href,{maxRedirects:0});
    const location = denied.headers().location;
    const loginRedirect = location ? new URL(location,base) : null;
    // Upstream LOGIN_URL is /login; Django may canonicalize it to /login/.
    // Require the same application's login and the exact protected destination.
    if (![301,302].includes(denied.status()) || !loginRedirect || loginRedirect.origin !== base.origin || !['/login','/login/'].includes(loginRedirect.pathname) || loginRedirect.searchParams.get('next') !== '/bookmarks') throw new Error('private bookmarks did not require login');
    const rejected = await login(randomBytes(24).toString('hex'));
    if (rejected.status() !== 401 || !(await rejected.text()).includes("Your username and password didn't match. Please try again.")) throw new Error('wrong login credentials were not rejected by the official form');
  }
  const accepted = await login(state.password);
  const acceptedLocation = accepted.headers().location;
  const destination = acceptedLocation ? new URL(acceptedLocation,base) : null;
  if (accepted.status() !== 302 || !destination || destination.origin !== base.origin || destination.pathname !== '/bookmarks') throw new Error('required administrator login did not create an authenticated session');
  await page.goto(new URL('/bookmarks/new',base).href);
  await expect(page.locator('#id_url'),'authenticated bookmark creation form').toBeVisible();
  if (phase === 'first-use') {
    const nonce = randomBytes(12).toString('hex');
    state = {...state,url:`https://example.com/?local-store-proof=${nonce}`,title:`Local Store proof ${nonce}`,description:'An exact synthetic private bookmark',notes:`Private saved notes ${nonce}`,tags:['local-store-proof',`proof-${nonce}`]};
    const csrf = await page.locator('form input[name=csrfmiddlewaretoken]').inputValue();
    const saved = await context.request.post(new URL('/bookmarks/new',base).href,{form:{csrfmiddlewaretoken:csrf,url:state.url,title:state.title,description:state.description,notes:state.notes,tag_string:state.tags.join(' '),auto_close:''},headers:{Referer:page.url()},maxRedirects:3,timeout:30000});
    if (!saved.ok()) throw new Error('exact private bookmark form was not saved');
  }
  await page.goto(new URL('/bookmarks',base).href);
  const bookmark = page.locator('.bookmark-list > li').filter({has:page.getByRole('link',{name:state.title,exact:true})});
  await expect(bookmark).toHaveCount(1);
  await expect(bookmark.getByRole('link',{name:state.title,exact:true})).toHaveAttribute('href',state.url);
  const bookmarkId = await bookmark.getAttribute('data-bookmark-id');
  if (!/^[1-9][0-9]{0,18}$/.test(bookmarkId)) throw new Error('saved bookmark identity is invalid');
  // The official edit route is stable even when the list's optional Edit
  // action is hidden by display preferences. Only this same saved row is read.
  const target = new URL(`/bookmarks/${bookmarkId}/edit`,base);
  await page.goto(target.href);
  await expect(page.locator('#id_url')).toHaveValue(state.url);
  await expect(page.locator('#id_title')).toHaveValue(state.title);
  await expect(page.locator('#id_description')).toHaveValue(state.description);
  await expect(page.locator('#id_notes')).toHaveValue(state.notes);
  const tags = (await page.locator('#id_tag_string').inputValue()).trim().split(/\s+/).sort();
  if (JSON.stringify(tags) !== JSON.stringify([...state.tags].sort())) throw new Error('private bookmark tags changed');
  await expect(page.locator('#id_shared')).not.toBeChecked();
  if (phase === 'first-use') await writeFile(statePath,JSON.stringify(state),{mode:0o600});
  console.log('Linkding exact private bookmark, notes, tags and login passed');
} finally { await browser.close(); }
