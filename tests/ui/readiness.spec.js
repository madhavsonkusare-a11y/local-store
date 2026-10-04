import {test, expect} from '@playwright/test';
import {installAdapter} from './fixtures.js';

const connection = {id:'notes', display_name:'Notes', launch_url:'http://localhost:5230',
  status:'connected', runtime:{kind:'external'}};
test.beforeEach(async ({page}) => {
  await page.clock.install();
  await page.emulateMedia({reducedMotion:'reduce'});
  await installAdapter(page, {apps:[connection]});
  await page.goto('/');
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__answer = 'ready'; window.__probes = 0;
    window.__TAURI__.core.invoke = (name, args) => {
      if (name !== 'app_readiness') return original(name, args);
      window.__probes++; return Promise.resolve(window.__answer);
    };
  });
});

test('live readiness changes without replacing the row or disturbing focus', async ({page}) => {
  await page.getByRole('button', {name:'My Apps'}).click();
  const row = page.locator('.installed-app[data-app-id="notes"]');
  await expect(row.locator('.status')).toHaveText('ready');
  await page.locator('#my-apps-detail').getByRole('button', {name:'Open', exact:true}).focus();
  await page.evaluate(() => { window.__row = document.querySelector('.installed-app'); window.__answer = 'unreachable'; });
  await page.clock.fastForward(16000);
  await expect(row.locator('.status')).toHaveText('not responding');
  await expect(page.locator('#my-apps-detail').getByRole('button', {name:'Open', exact:true})).toBeFocused();
  expect(await page.evaluate(() => window.__row === document.querySelector('.installed-app'))).toBe(true);
  await page.evaluate(() => { window.__answer = 'unknown'; });
  await page.clock.fastForward(16000);
  await expect(row.locator('.status')).toHaveText('connected');
});

test('leaving My Apps and hiding the launcher pauses readiness checks', async ({page}) => {
  await page.getByRole('button', {name:'My Apps'}).click();
  await expect(page.locator('.installed-app .status')).toHaveText('ready');
  await page.getByRole('button', {name:'Discover', exact:true}).click();
  const before = await page.evaluate(() => window.__probes);
  await page.clock.fastForward(60000);
  expect(await page.evaluate(() => window.__probes)).toBe(before);
  await page.getByRole('button', {name:'My Apps'}).click();
  await expect(page.locator('.installed-app .status')).toHaveText('ready');
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', {configurable:true, get:() => true});
    document.dispatchEvent(new Event('visibilitychange'));
  });
  const hidden = await page.evaluate(() => window.__probes);
  await page.clock.fastForward(60000);
  expect(await page.evaluate(() => window.__probes)).toBe(hidden);
  await page.evaluate(() => {
    delete document.hidden; window.__answer = 'unreachable';
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('.installed-app .status')).toHaveText('not responding');
});

test('connection removal can fail and retry without duplicate requests or lost focus', async ({page}) => {
  await page.getByRole('button', {name:'My Apps'}).click();
  await page.getByRole('button', {name:'Remove Notes'}).click();
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__removes = 0;
    window.__TAURI__.core.invoke = (name, args) => {
      if (name === 'remove_app_cmd' && ++window.__removes === 1)
        return new Promise((resolve, reject) => { window.__failRemove = reject; });
      return original(name, args);
    };
  });
  const confirm = page.getByRole('button', {name:'Remove connection', exact:true});
  await confirm.click();
  await expect(confirm).toBeDisabled();
  await page.keyboard.press('Escape');
  await expect(page.locator('#remove-dialog')).toBeVisible();
  await page.evaluate(() => window.__failRemove({code:'storage_io', message:'Could not save the registry.'}));
  await expect(page.locator('#remove-error')).toContainText('Could not save the registry.');
  await expect(confirm).toBeEnabled();
  await confirm.click();
  await expect(page.getByRole('heading', {name:'Your apps belong here.'})).toBeVisible();
  await expect(page.locator('#content button')).toBeFocused();
  expect(await page.evaluate(() => window.__removes)).toBe(2);
  expect(await page.evaluate(() => window.__calls.some(call => call.command === 'uninstall_app'))).toBe(false);
  const count = await page.evaluate(() => window.__probes);
  await page.clock.fastForward(60000);
  expect(await page.evaluate(() => window.__probes)).toBe(count);
});
