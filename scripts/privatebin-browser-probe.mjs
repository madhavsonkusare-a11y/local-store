// Opt-in browser probe for synthetic qualification data, using existing Playwright.
import { chromium, expect } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';

const [mode, endpoint, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(mode) || !statePath) throw new Error('first-use|verify URL STATE required');
const target = new URL(endpoint);
if (target.protocol !== 'http:' || !['127.0.0.1', 'localhost', '[::1]'].includes(target.hostname)) throw new Error('loopback only');
const browser = await chromium.launch({ headless: true, channel: 'msedge' });
try {
  const page = await browser.newPage();
  page.setDefaultTimeout(30000);
  const marker = 'Local Store encrypted browser qualification';
  if (mode === 'first-use') {
    await page.goto(endpoint);
    if (!await page.evaluate(() => Boolean(window.isSecureContext && window.crypto.subtle))) {
      throw new Error('loopback browser crypto unavailable');
    }
    await page.locator('#message').fill(marker);
    await page.locator('#sendbutton:visible').click();
    await page.waitForFunction(() => Boolean(location.search && location.hash));
    const url = page.url();
    await writeFile(statePath, JSON.stringify({ url }));
    await page.goto(url);
  } else {
    const { url } = JSON.parse(await readFile(statePath, 'utf8'));
    if (new URL(url).origin !== target.origin) throw new Error('paste origin changed');
    await page.goto(url);
  }
  await expect(page.locator('#prettyprint')).toContainText(marker);
  console.log('Encrypted paste browser round trip passed');
} finally {
  await browser.close();
}
