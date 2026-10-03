import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

test('Settings is a workspace route with heading focus, live navigation and browser back', async ({page}) => {
  await installAdapter(page); await page.goto('/');
  await page.getByRole('button',{name:'Overview',exact:true}).click();
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await expect(page).toHaveURL(/#settings$/);
  await expect(page.getByRole('heading',{name:'Settings',exact:true})).toBeFocused();
  await expect(page.locator('#settings')).toHaveAttribute('aria-current','page');
  expect(await page.locator('#settings-dialog').evaluate(node=>node.open && !node.matches(':modal'))).toBe(true);
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.getByRole('button',{name:'Activity',exact:true}).click();
  await expect(page).toHaveURL(/#activity$/);
  await expect(page.locator('#settings-dialog')).not.toBeVisible();
  await page.goBack();
  await expect(page).toHaveURL(/#settings$/);
  await expect(page.getByRole('heading',{name:'Settings',exact:true})).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('#settings-dialog')).not.toBeVisible();
});

test('routed engine setup still needs explicit consent and holds navigation while committing', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    const engine={bootstrap:{status:'not_configured'},prerequisites:{state:'ready'}};
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='engine_setup_preview') return Promise.resolve({engine,payload:'verified_development',development_engine_available:true,required_disk_bytes:1073741824,disk_sufficient:true,payload_release_approved:false});
      if(command==='engine_setup_action') {window.__calls.push({command,args});return new Promise(resolve=>{window.__finishSetup=resolve;});}
      return original(command,args);
    };
  });
  await page.goto('/#settings');
  const start=page.locator('#engine-setup-start'); await expect(start).toBeVisible();
  await start.click();
  await expect(page.locator('#engine-error')).toContainText('Review and accept each');
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='engine_setup_action'))).toBe(false);
  for(const checkbox of await page.locator('[data-engine-consent]').all()) await checkbox.check();
  await start.click();
  await page.getByRole('button',{name:'Discover',exact:true}).click();
  await expect(page).toHaveURL(/#settings$/);
  await expect(page.locator('#settings-dialog')).toBeVisible();
  await page.evaluate(()=>window.__finishSetup());
  await expect(page.locator('#settings-dialog')).toHaveAttribute('aria-busy','false');
  await page.getByRole('button',{name:'Discover',exact:true}).click();
  await expect(page.locator('#settings-dialog')).not.toBeVisible();
  await expect(page.locator('#page-title')).toBeFocused();
});
