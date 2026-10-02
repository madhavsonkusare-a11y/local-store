import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

async function introduction(page) {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => {
      if (command === 'onboarding_progress') return Promise.resolve({viewed:[]});
      return original(command,args);
    };
  });
  await page.goto('/');
  await page.getByRole('button',{name:'Get started',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Choose your first app'})).toBeFocused();
}
test('starter selection preserves chosen port into a review without installing or granting consent', async ({page}) => {
  await introduction(page);
  await page.getByRole('radio',{name:/n8n/}).check();
  await expect(page.getByLabel('Published port')).toHaveValue('5678');
  await page.getByLabel('Published port').fill('5680');
  await page.getByRole('button',{name:'Review installation',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Review n8n installation'})).toBeVisible();
  await expect(page.locator('#recipe-port')).toHaveValue('5680');
  await expect(page.locator('#recipe-address')).toContainText(':5680');
  expect(await page.evaluate(() => window.__calls.some(call => ['install_app','engine_setup_action','agent_grant_set'].includes(call.command)))).toBe(false);
});
test('invalid starter port remains in choice and ready choices are accessible', async ({page}) => {
  await introduction(page);
  await expect(page.getByRole('radio',{name:/Memos/})).toBeChecked();
  await page.getByLabel('Published port').fill('80');
  await page.getByRole('button',{name:'Review installation',exact:true}).click();
  await expect(page.locator('#first-run-error')).toContainText('1024 and 65535');
  await expect(page.getByLabel('Published port')).toBeFocused();
  await expect(page.getByRole('spinbutton',{name:'Published port',exact:true})).toBeInViewport();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.screenshot({path:'.cache/first-run-1280.png'});
  await page.setViewportSize({width:800,height:600});
  await expect(page.getByRole('button',{name:'Review installation',exact:true})).toBeInViewport();
  expect(await page.evaluate(() => document.getElementById('first-run-dialog').scrollWidth <= document.getElementById('first-run-dialog').clientWidth)).toBe(true);
  await page.screenshot({path:'.cache/first-run-800.png'});
});
test('back from starter review keeps the chosen app and port without granting access', async ({page}) => {
  await introduction(page);
  await page.getByRole('radio',{name:/n8n/}).check();
  await page.getByLabel('Published port').fill('5681');
  await page.getByRole('button',{name:'Review installation',exact:true}).click();
  await page.getByRole('button',{name:'Back to starter choices',exact:true}).click();
  await expect(page.getByRole('radio',{name:/n8n/})).toBeChecked();
  await expect(page.getByRole('spinbutton',{name:'Published port',exact:true})).toHaveValue('5681');
  await expect(page.getByRole('spinbutton',{name:'Published port',exact:true})).toBeFocused();
  expect(await page.evaluate(() => window.__calls.some(call => ['install_app','agent_grant_set'].includes(call.command)))).toBe(false);
});
test('a completed introduction can be reopened from Settings', async ({page}) => {
  await installAdapter(page);
  await page.goto('/');
  await page.locator('#settings').click();
  await page.getByRole('button',{name:'Run introduction',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Your apps. Your computer.'})).toBeVisible();
  await expect(page.locator('.first-run-rail [aria-current="step"]')).toHaveText('Welcome');
});

test('back to welcome keeps keyboard focus inside the visible step', async ({page}) => {
  await introduction(page);
  await page.getByRole('button',{name:'Back to welcome',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Your apps. Your computer.'})).toBeFocused();
});
test('success shows actual saved facts and opening needs an explicit action', async ({page}) => {
  await installAdapter(page);
  await page.goto('/');
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button',{name:'Install Memos',exact:true}).click();
  const result = page.getByRole('dialog',{name:'Memos installed'});
  await expect(result).toBeVisible();
  await expect(result).toContainText('running');
  await expect(result).toContainText('http://localhost:5230');
  expect(await page.evaluate(() => window.__calls.filter(call => call.command === 'open_app'))).toHaveLength(0);
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.getByRole('button',{name:'Open app',exact:true}).click();
  await expect(result).not.toBeVisible();
  expect(await page.evaluate(() => window.__calls.find(call => call.command === 'open_app'))).toEqual({command:'open_app',args:{id:'memos'}});
});
test('open failure retains successful installation and offers a retry', async ({page}) => {
  await installAdapter(page,{failure:'open_app'});
  await page.goto('/');
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button',{name:'Install Memos',exact:true}).click();
  await page.getByRole('button',{name:'Open app',exact:true}).click();
  await expect(page.locator('#install-result-error')).toContainText('Could not open app');
  await expect(page.getByRole('button',{name:'Open app',exact:true})).toBeEnabled();
  expect(await page.evaluate(() => window.__calls.filter(call => call.command === 'install_app'))).toHaveLength(1);
});
