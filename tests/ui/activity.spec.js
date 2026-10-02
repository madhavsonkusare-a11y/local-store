import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

test('Activity settles from IPC, keeps keyboard context and clears on reload', async ({page}) => {
  await installAdapter(page);
  await page.goto('/');
  await page.evaluate(()=>{
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>command==='open_app' ? new Promise(resolve=>{window.__resolveOpen=resolve;}) : original(command,args);
  });
  await page.locator('#nav-apps').click(); await page.locator('[data-open]').click(); await page.locator('#nav-activity').click();
  await expect(page.locator('#activity-title')).toBeFocused();
  await expect(page.locator('#activity-content')).toContainText('Open · Studio notes');
  await expect(page.locator('#activity-content')).toContainText('In progress');
  await page.evaluate(()=>window.__operationListener({payload:{operation_id:'test-open',app_id:'studio-notes',kind:'open',state:'started'}}));
  await page.evaluate(()=>window.__operationListener({payload:{operation_id:'test-open',app_id:'studio-notes',kind:'open',state:'succeeded'}}));
  await expect(page.locator('#activity-content')).toContainText('Finishing');
  const summary=page.locator('.activity-event summary'); await summary.focus(); await page.keyboard.press('Enter');
  await page.evaluate(()=>window.__resolveOpen());
  await expect(page.locator('#activity-content')).toContainText('Completed');
  await expect(page.locator('.activity-event')).toHaveAttribute('open','');
  await expect(page.locator('.activity-event summary')).toBeFocused();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.reload(); await page.locator('#nav-activity').click();
  await expect(page.locator('#activity-content')).toContainText('A quiet session so far');
});

test('backend activity is bounded and excludes arguments, credentials and error messages', async ({page}) => {
  await installAdapter(page); await page.goto('/'); await page.locator('#nav-activity').click();
  await page.evaluate(()=>{
    for(let index=0;index<105;index++) window.__operationListener({payload:{operation_id:`operation-${index}`,app_id:'memos',kind:'install',state:'failed',error:{code:'invalid_input',message:'PRIVATE-CREDENTIAL-DO-NOT-RETAIN'},answers:{API_KEY:'PRIVATE-KEY'},stage:'not-valid'}});
  });
  await expect(page.locator('.activity-event')).toHaveCount(100);
  await expect(page.locator('#activity-content')).not.toContainText('PRIVATE');
  await page.getByRole('button',{name:'Failed',exact:true}).click();
  await expect(page.locator('.activity-event')).toHaveCount(100);
  await page.locator('#activity-search').fill('other-app');
  await expect(page.locator('#activity-content')).toContainText('No matching operations');
});

test('agent navigation opens real owner connection controls without automatic permissions', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(()=>{
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='agent_connections') {window.__calls.push({command,args});return Promise.resolve({clients:[],apps:[],grants:[],audit:[],sidecar_available:true,same_user_limitation:'Permissions are explicit.'});}
      if(command==='agent_mutation_requests') return Promise.resolve([]);
      return original(command,args);
    };
  });
  await page.goto('/'); await page.locator('#nav-agents').click();
  await expect(page.locator('#settings-dialog')).toBeVisible();
  await expect(page.locator('#agent-panel')).toBeVisible();
  await expect(page.locator('#agent-client-name')).toBeFocused();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='agent_grant_set'||call.command==='agent_enrollment_prepare'))).toBe(false);
});
