import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

const apps = [
  {id:'memos', display_name:'Memos', launch_url:'http://localhost:5230', runtime:{kind:'compose'}, status:'running', catalog_id:'Memos', created_at_unix:1780000000, updated_at_unix:1780000200},
  {id:'photos', display_name:'Photos', launch_url:'http://192.168.1.5:2283', runtime:{kind:'external'}, status:'connected', created_at_unix:1780000000, updated_at_unix:1780000200},
];

test.beforeEach(async ({page}) => { await installAdapter(page, {apps}); await page.goto('/'); await page.getByRole('button', {name:/My Apps/}).click(); });

test('selection, live details, and managed logs are usable from keyboard', async ({page}) => {
  await expect(page.locator('.my-apps-layout')).toBeVisible();
  await expect(page.locator('#my-apps-detail-title')).toHaveText('Memos');
  await expect(page.getByRole('button', {name:'Select Memos'})).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button', {name:'Select Memos'}).focus();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('button', {name:'Select Photos'})).toBeFocused();
  await expect(page.locator('#my-apps-detail-title')).toHaveText('Photos');
  await expect(page.getByRole('tab', {name:'Logs'})).toHaveCount(0);
  await expect(page.locator('.my-apps-detail-body')).toContainText('does not start or manage the external server');
  await page.getByRole('button', {name:'Select Memos'}).click();
  await page.getByRole('tab', {name:'Logs'}).click();
  await expect(page.locator('.my-apps-detail .log-view')).toContainText('server started');
  expect(await page.evaluate(() => window.__calls.filter(call => call.command === 'app_logs'))).toEqual([{command:'app_logs',args:{id:'memos'}}]);
  await page.getByRole('tab', {name:'Logs'}).focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', {name:'Manage'})).toBeFocused();
  await expect(page.locator('.my-apps-manage')).toContainText('Uninstall keeps managed data by default');
  const results = await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa']).analyze();
  expect(results.violations).toEqual([]);
});

test('a failed refresh retains last-known apps and selected identity', async ({page}) => {
  await page.getByRole('button', {name:'Select Photos'}).click();
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command, args) => command === 'list_apps' ? Promise.reject(new Error('Temporary read failure')) : original(command, args);
  });
  await page.getByRole('button', {name:/My Apps/}).click();
  await expect(page.getByText('App refresh failed.')).toBeVisible();
  await expect(page.locator('#my-apps-detail-title')).toHaveText('Photos');
  await expect(page.locator('.installed-app')).toHaveCount(2);
});

test('Manage acts on the selected app and respects pending operations', async ({page}) => {
  await page.getByRole('button', {name:'Select Photos'}).click();
  await page.getByRole('tab', {name:'Manage'}).click();
  const panel = page.locator('#my-apps-panel');
  await panel.getByRole('button', {name:'Open', exact:true}).click();
  expect(await page.evaluate(() => window.__calls.find(call => call.command === 'open_app'))).toEqual({command:'open_app',args:{id:'photos'}});
  await expect(panel.getByRole('button', {name:'Start', exact:true})).toHaveCount(0);
  await page.getByRole('button', {name:'Select Memos'}).click();
  await page.getByRole('tab', {name:'Manage'}).click();
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => command === 'stop_app' ? new Promise(resolve => {window.__finishStop = resolve;}) : original(command,args);
  });
  await panel.getByRole('button', {name:'Stop', exact:true}).click();
  await expect(panel.getByRole('button', {name:'Stop', exact:true})).toHaveAttribute('aria-disabled','true');
  await page.evaluate(() => window.__finishStop());
  await expect(panel.getByRole('button', {name:'Stop', exact:true})).not.toHaveAttribute('aria-disabled','true');
});

test('log copying is explicit and clipboard refusal preserves selectable logs', async ({page}) => {
  await page.evaluate(() => {
    window.__copiedLogs = [];
    Object.defineProperty(navigator, 'clipboard', {configurable:true, value:{writeText:async text => {window.__copiedLogs.push(text);}}});
  });
  await page.getByRole('tab', {name:'Logs'}).click();
  await expect(page.locator('.my-apps-detail .log-view')).toContainText('server started');
  expect(await page.evaluate(() => window.__copiedLogs)).toEqual([]);
  await page.getByRole('button', {name:'Copy logs', exact:true}).click();
  expect(await page.evaluate(() => window.__copiedLogs)).toEqual(['memos  | server started on port 5230']);
  await expect(page.getByRole('button', {name:'Copy logs', exact:true})).toBeFocused();
  await page.evaluate(() => {navigator.clipboard.writeText = async () => {throw new Error('Clipboard refused');};});
  await page.getByRole('button', {name:'Copy logs', exact:true}).click();
  await expect(page.locator('#my-apps-panel')).toContainText('Clipboard refused');
  await expect(page.locator('.my-apps-detail .log-view')).toContainText('server started');
});
