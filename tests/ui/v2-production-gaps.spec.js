import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

const managed = {id:'memos', display_name:'Memos', launch_url:'http://localhost:5230', runtime:{kind:'compose'}, status:'running', catalog_id:'Memos'};

test('deleting managed data requires the exact selected app name', async ({page}) => {
  await installAdapter(page,{apps:[managed]}); await page.goto('/');
  await page.getByRole('button',{name:/My Apps/}).click();
  await page.getByRole('tab',{name:'Manage'}).click();
  await page.locator('#my-apps-panel').getByRole('button',{name:'Uninstall Memos',exact:true}).click();
  await expect(page.locator('#uninstall-confirm')).toBeEnabled();
  await page.getByLabel('Delete app data too').check();
  await expect(page.locator('#delete-name')).toBeFocused();
  await expect(page.locator('#uninstall-confirm')).toBeDisabled();
  await page.locator('#delete-name').fill('memos');
  await expect(page.locator('#uninstall-confirm')).toBeDisabled();
  expect(await page.evaluate(() => window.__calls.some(call => call.command === 'uninstall_app'))).toBe(false);
  await page.locator('#delete-name').fill('Memos');
  await page.locator('#uninstall-confirm').click();
  await expect(page.locator('#uninstall-dialog')).not.toBeVisible();
  expect(await page.evaluate(() => window.__calls.filter(call => call.command === 'uninstall_app'))).toEqual([{command:'uninstall_app',args:{id:'memos',deleteData:true}}]);
});

test('Overview recent activity reflects a real request and links to session history', async ({page}) => {
  await installAdapter(page); await page.goto('/');
  await page.getByRole('button',{name:/My Apps/}).click();
  await page.locator('#my-apps-detail').getByRole('button',{name:'Open',exact:true}).click();
  await page.getByRole('button',{name:'Overview',exact:true}).click();
  await expect(page.locator('#overview-recent-rows')).toContainText('Open');
  await expect(page.locator('#overview-recent-rows')).toContainText('Studio notes');
  await expect(page.locator('#overview-recent-rows')).toContainText('Completed');
  await page.getByRole('button',{name:'View this session',exact:true}).click();
  await expect(page.locator('#activity-content')).toContainText('Open · Studio notes');
  expect(await page.evaluate(() => window.__calls.filter(call => call.command === 'open_app'))).toEqual([{command:'open_app',args:{id:'studio-notes'}}]);
});

test('first-run preflight and Install to Ready steps preserve explicit consent at the approved minimum size', async ({page}) => {
  await page.setViewportSize({width:1180,height:640});
  await installAdapter(page,{apps:[]});
  await page.addInitScript(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => command === 'onboarding_progress' ? Promise.resolve({viewed:[]}) : original(command,args);
  });
  await page.goto('/');
  await expect(page.locator('#welcome-engine-checks')).toContainText('Local engine ready.');
  await expect(page.getByRole('heading',{name:'Your apps. Your computer.'})).toBeFocused();
  await expect(page.getByRole('button',{name:'Get started',exact:true})).toBeInViewport();
  await page.screenshot({path:'.cache/v2-first-run-welcome-1180.png'});
  await page.getByRole('button',{name:'Get started',exact:true}).click();
  await expect(page.locator('#welcome-engine-checks')).not.toBeVisible();
  await page.getByRole('button',{name:'Review installation',exact:true}).click();
  await expect(page.locator('#install-dialog .first-run-task-rail [aria-current="step"]')).toHaveText('Install');
  expect(await page.evaluate(() => window.__calls.some(call => ['install_app','engine_setup_action','agent_grant_set'].includes(call.command)))).toBe(false);
  await page.getByRole('button',{name:'Install Memos',exact:true}).click();
  const result = page.getByRole('dialog',{name:'Memos installed'});
  await expect(result.locator('.first-run-task-rail [aria-current="step"]')).toHaveText('Ready');
  await expect(result).toContainText('http://localhost:5230');
  await expect(page.getByRole('button',{name:'Open app',exact:true})).toBeInViewport();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  expect(await page.evaluate(() => window.__calls.some(call => ['open_app','engine_setup_action','agent_grant_set'].includes(call.command)))).toBe(false);
  await page.screenshot({path:'.cache/v2-first-run-ready-1180.png'});
});


test('Load next24 retains existing projects and failure preserves a retry without stale search results', async ({page}) => {
  const catalog = Array.from({length:55},(_,index) => ({name:`Project ${String(index).padStart(2,'0')}`,description:'An independent project.',category:'Tools',license:'MIT',capability:'connect'}));
  await installAdapter(page,{catalog}); await page.goto('/');
  await expect(page.locator('.app-card')).toHaveCount(24);
  const originalNames = await page.locator('.app-card h3').allTextContents();
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke; let failed = false;
    window.__TAURI__.core.invoke = (command,args) => {
      if (command === 'search_catalog' && args.offset === 24 && !failed) {failed = true; return Promise.reject(new Error('Private unavailable reason'));}
      return original(command,args);
    };
  });
  const next = page.getByRole('button',{name:'Load next 24',exact:true});
  await next.click();
  await expect(page.locator('#toast')).toContainText('Your current results are still here');
  await expect(page.locator('.app-card')).toHaveCount(24);
  await expect(next).toBeVisible(); await next.click();
  await expect(page.locator('.app-card')).toHaveCount(48);
  expect((await page.locator('.app-card h3').allTextContents()).slice(0,24)).toEqual(originalNames);
  await page.getByRole('button',{name:'View Project 47 details',exact:true}).click();
  await expect(page.locator('#detail-title')).toHaveText('Project 47');
  await page.getByRole('button',{name:'Close app details'}).click();
  await expect(page.locator('#detail-dialog')).not.toBeVisible();
  await page.getByRole('searchbox',{name:'Search apps',exact:true}).fill('Project 54');
  await expect(page.locator('.app-card')).toHaveCount(1);
  await expect(page.locator('#page-label')).toHaveText('Showing 1 matching projects');
  await expect(next).toBeHidden();
  const styles = await page.locator('#connect-top').evaluate(button => ({font:getComputedStyle(button).fontSize,radius:getComputedStyle(button).borderRadius,height:button.getBoundingClientRect().height}));
  expect(styles).toEqual({font:'14px',radius:'999px',height:42});
  await page.setViewportSize({width:1280,height:800});
  await page.getByRole('searchbox',{name:'Search apps',exact:true}).fill('');
  await expect(page.locator('.app-card')).toHaveCount(24);
  await page.evaluate(() => {window.scrollTo(0,0); document.getElementById('toast').hidden = true;});
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({path:'.cache/v2-discover-primitives-1280.png'});
});
