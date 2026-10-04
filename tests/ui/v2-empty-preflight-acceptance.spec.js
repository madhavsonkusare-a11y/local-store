import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

for(const route of ['overview','discover','my-apps']) test(`V2 ${route} empty state remains accessible with useful next actions`,async({page})=>{
  await installAdapter(page,{apps:[],catalog:[]});
  await page.setViewportSize({width:1280,height:640});
  await page.goto(`/#${route}`);
  await expect(page.locator('[aria-busy="true"]:visible')).toHaveCount(0);
  if(route==='overview') {
    await expect(page.locator('.overview-metrics strong')).toHaveText(['0','0','0','0']);
    await expect(page.locator('#overview-content')).toContainText('No apps saved yet');
  } else if(route==='discover') await expect(page.locator('#content')).toContainText('Nothing here just yet.');
  else await expect(page.locator('#content')).toContainText('Your apps belong here.');
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.locator('#settings').focus(); await page.keyboard.press('Enter');
  await expect(page.locator('#settings-title')).toBeFocused();
});

test('unavailable installation review cannot commit and has keyboard escape',async({page})=>{
  await installAdapter(page,{failure:'recipe_details'}); await page.goto('/');
  await page.locator('[data-featured="memos"]').click();
  await expect(page.locator('#install-error')).toContainText('Could not recipe details');
  await expect(page.locator('#install-confirm')).toBeDisabled();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(page.locator('#install-dialog')).toBeHidden();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='install_app'))).toBe(false);
});

for(const mode of ['checking','ready','missing']) test(`first-run ${mode} uses the actual preflight and never assumes consent`,async({page})=>{
  await installAdapter(page);
  await page.addInitScript(mode=>{
    const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='onboarding_progress') return Promise.resolve({viewed:[]});
      if(command==='doctor') {
        window.__calls.push({command,args});
        if(mode==='checking') return new Promise(resolve=>{window.__preflightResolve=resolve;});
        return Promise.resolve({ready:mode==='ready',checks:[]});
      }
      return invoke(command,args);
    };
  },mode);
  await page.goto('/');
  await expect(page.locator('#first-run-dialog')).toBeVisible();
  await expect(page.locator('#welcome-engine-checks')).toContainText(mode==='checking'?'Checking':mode==='ready'?'Local engine ready':'needs setup');
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  expect(await page.evaluate(()=>window.__calls.some(call=>['engine_setup_action','install_app','agent_grant_set'].includes(call.command)))).toBe(false);
});
