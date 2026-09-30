// Opt-in browser proof for Uptime Kuma 2.5.3; synthetic credentials and monitor.
import { chromium, expect } from '@playwright/test';
import { randomBytes, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [mode, endpoint, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(mode) || !statePath) throw new Error('first-use|verify URL STATE required');
const base = new URL(endpoint);
if (base.protocol !== 'http:' || !['127.0.0.1', 'localhost', '[::1]'].includes(base.hostname)) throw new Error('loopback only');
if (base.hostname === 'localhost') base.hostname = '127.0.0.1';
const state = mode === 'first-use'
  ? { username: `localstore${randomBytes(3).toString('hex')}`, password: randomBytes(18).toString('base64url'), name: `Local Store ${randomUUID()}` }
  : JSON.parse(await readFile(statePath, 'utf8'));
const browser = await chromium.launch({ headless: true, channel: 'msedge' });
try {
  const page = await browser.newPage();
  page.setDefaultTimeout(30000);
  await page.goto(base.href);
  if (mode === 'first-use' && new URL(page.url()).pathname === '/setup-database') {
    await page.locator('label[for="btnradio1"]').click();
    await page.locator('button[type="submit"]').click();
  }
  try {
    await page.locator('[data-cy="setup-form"], form[aria-label="Login Form"], [data-testid="monitor-list"]').first().waitFor({ timeout: 60000 });
  } catch (error) {
    await page.screenshot({ path: `${statePath}.png` });
    await writeFile(`${statePath}.html`, await page.content());
    throw new Error(`Initial Uptime Kuma screen unavailable at ${page.url()}: ${error.message}`);
  }

  if (mode === 'first-use') {
    const setup = page.locator('[data-cy="setup-form"]');
    await expect(setup).toBeVisible();
    await setup.locator('[data-cy="username-input"]').fill(state.username);
    await setup.locator('[data-cy="password-input"]').fill(state.password);
    await setup.locator('[data-cy="password-repeat-input"]').fill(state.password);
    await setup.locator('[data-cy="submit-setup-form"]').click();
    await writeFile(statePath, JSON.stringify(state));
  } else {
    const login = page.locator('form[aria-label="Login Form"]');
    if (await login.isVisible()) {
      await login.locator('#floatingInput').fill(state.username);
      await login.locator('#floatingPassword').fill(state.password);
      await login.locator('button[type="submit"]').click();
    }
  }

  await page.locator('[data-testid="monitor-list"]').waitFor();
  if (mode === 'first-use') {
    await page.goto(new URL('/add', base).href);
    await page.getByTestId('friendly-name-input').fill(state.name);
    await page.getByTestId('url-input').fill('http://127.0.0.1:3001/');
    await page.getByTestId('save-button').click();
  } else {
    await page.getByTestId('monitor-list').getByText(state.name, { exact: true }).click();
  }

  await expect(page.getByTestId('monitor-list').getByText(state.name, { exact: true })).toBeVisible();
  await expect(page.getByTestId('monitor-status')).toHaveText(/up/i, { timeout: 90000 });
  console.log('Exact self-monitor survived and recorded an Up heartbeat');
} finally {
  await browser.close();
}
