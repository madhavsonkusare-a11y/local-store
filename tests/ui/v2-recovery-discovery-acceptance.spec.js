import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

for(const ownership of ['verified','no_containers']) test(`recovery ${ownership} keeps data by default and reports only confirmed cleanup`,async({page})=>{
  await page.setViewportSize({width:1280,height:640});
  await installAdapter(page);
  await page.addInitScript(ownership=>{
    const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='inspect_recovery') return Promise.resolve([{recipe_id:'memos',display_name:'Memos',ownership_status:ownership,docker_ownership_verified:ownership==='verified',compose_file:'D:/retained/memos/compose.yaml'}]);
      if(command==='discard_retained_setup') {window.__calls.push({command,args});return new Promise(resolve=>{window.__cleanupResolve=resolve;});}
      return invoke(command,args);
    };
  },ownership);
  await page.goto('/#settings'); await page.locator('#scan-recovery').click();
  if(ownership==='no_containers') await expect(page.locator('#recovery-output')).toContainText('no matching container');
  await page.getByRole('button',{name:'Review setup',exact:true}).click();
  await expect(page.locator('#recovery-title')).toBeFocused();
  await expect(page.locator('#recovery-delete-data')).not.toBeChecked();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.locator('#recovery-clear').click();
  await expect(page.locator('#recovery-clear')).toBeDisabled();
  await expect(page.locator('#recovery-task-status')).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('#recovery-task')).toBeVisible();
  await page.evaluate(()=>window.__cleanupResolve({containers_removed:0,data_deleted:false}));
  await expect(page.locator('#recovery-task-status')).toContainText('0 owned containers removed. Managed data was kept.');
  await expect(page.locator('#recovery-clear')).toBeHidden();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  expect(await page.evaluate(()=>window.__calls.find(call=>call.command==='discard_retained_setup').args)).toEqual({recipeId:'memos',deleteData:false});
});

for(const mode of ['loading','failure']) test(`Discover initial ${mode} has accessible feedback and never shows fabricated results`,async({page})=>{
  await installAdapter(page);
  await page.addInitScript(mode=>{
    const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='search_catalog') {
        window.__calls.push({command,args});
        if(mode==='failure') return Promise.reject({code:'timed_out',message:'Catalog unavailable'});
        return new Promise(resolve=>{window.__catalogResolve=resolve;});
      }
      return invoke(command,args);
    };
  },mode);
  await page.goto('/');
  if(mode==='loading') {
    await expect(page.locator('#content')).toHaveAttribute('aria-busy','true');
    await expect(page.locator('#content .skeleton').first()).toBeVisible();
  } else {
    await expect(page.locator('#content')).toContainText('Catalog unavailable');
    await expect(page.getByRole('button',{name:'Try again',exact:true})).toBeEnabled();
  }
  await expect(page.locator('#content .app-card')).toHaveCount(0);
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.locator('#settings').focus();await page.keyboard.press('Enter');
  await expect(page.locator('#settings-title')).toBeFocused();
});
