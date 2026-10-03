import {test,expect} from '@playwright/test';
import {installAdapter} from './fixtures.js';

test('missing engine can connect a first app and shows actual saved facts without setup or launch', async ({page}) => {
  await page.setViewportSize({width:1180,height:640});
  await installAdapter(page,{apps:[]});
  await page.addInitScript(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => {
      if(command==='onboarding_progress') return Promise.resolve({viewed:[]});
      if(command==='doctor') return Promise.resolve({ready:false,checks:[{label:'Local engine',ok:false,detail:'Not set up'},{label:'Compose',ok:false,detail:'Waiting for the engine'}]});
      return original(command,args);
    };
  });
  await page.goto('/');
  await expect(page.locator('#welcome-engine-checks')).toContainText('needs setup');
  await page.getByRole('button',{name:'Connect an existing app',exact:true}).click();
  await expect(page.locator('#connect-dialog')).toBeVisible();
  await page.locator('#connect-name').fill('My dashboard');
  await page.locator('#connect-url').fill('http://127.0.0.1:9000');
  await page.locator('#connect-submit').click();
  const result=page.getByRole('dialog',{name:'My dashboard connected'});
  await expect(result).toBeVisible();
  await expect(result).toHaveCSS('opacity','1');
  await expect(result.locator('.first-run-task-rail [aria-current="step"]')).toHaveText('Ready');
  await expect(result).toContainText('http://127.0.0.1:9000');
  await expect(result).toContainText('availability not checked');
  await expect(result.getByRole('button',{name:'Open app',exact:true})).not.toBeVisible();
  await expect(result.getByRole('button',{name:'Enter workspace',exact:true})).toBeInViewport();
  expect(await page.evaluate(() => window.__calls.some(call=>['doctor_setup','engine_setup_action','install_app','open_app','agent_grant_set'].includes(call.command)))).toBe(false);
  await page.screenshot({path:'.cache/v2-first-run-connected-1180.png'});
  await result.getByRole('button',{name:'Enter workspace',exact:true}).click();
  await expect(result).not.toBeVisible();
  await expect(page.getByRole('button',{name:'Select My dashboard'})).toBeVisible();
});

test('cancelling the first-run connection returns to Welcome and saves nothing', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>command==='onboarding_progress'?Promise.resolve({viewed:[]}):original(command,args);
  });
  await page.goto('/');
  await page.getByRole('button',{name:'Connect an existing app',exact:true}).click();
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
  await expect(page.locator('#first-run-dialog')).toBeVisible();
  await expect(page.getByRole('heading',{name:'Your apps. Your computer.'})).toBeFocused();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='add_app'))).toBe(false);
});
