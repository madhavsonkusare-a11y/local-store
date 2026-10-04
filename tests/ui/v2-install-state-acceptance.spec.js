import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

async function pendingInstall(page) {
  await installAdapter(page,{apps:[]});
  await page.addInitScript(()=>{
    const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='install_app') {
        window.__calls.push({command,args});
        return new Promise((resolve,reject)=>{window.__finishInstall=resolve;window.__failInstall=reject;});
      }
      return invoke(command,args);
    };
  });
  await page.goto('/');
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button',{name:'Install Memos',exact:true}).click();
  await expect(page.locator('#install-progress')).toBeVisible();
}

const stages=['checking_system','preparing_files','validating_recipe','starting_containers','waiting_for_health','saving_app','rolling_back'];
for(const stage of stages) test(`V2 install stage ${stage} has an accessible factual current step`,async({page})=>{
  await page.setViewportSize({width:1280,height:640});
  await pendingInstall(page);
  await page.evaluate(stage=>{
    const event={app_id:'memos',kind:'install',operation_id:'state-proof'};
    window.__operationListener({payload:{...event,state:'started',cancel_id:42}});
    window.__operationListener({payload:{...event,state:'progress',stage}});
  },stage);
  await expect(page.locator('.install-stage-list [aria-current="step"]')).toHaveCount(1);
  await expect(page.locator('#install-confirm')).toBeDisabled();
  expect(await page.locator('#install-progress').innerText()).not.toMatch(/\d+%/);
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  const footer=await page.locator('#install-confirm').boundingBox();
  expect(footer.y+footer.height).toBeLessThanOrEqual(640);
  await page.keyboard.press('Escape');
  await expect(page.locator('#install-dialog')).toBeVisible();
  expect(await page.evaluate(()=>window.__calls.filter(call=>call.command==='install_app').length)).toBe(1);
});

for(const code of ['port_in_use','prerequisite_unavailable','invalid_input','timed_out','rollback_failed','cancelled']) {
  test(`V2 install ${code} preserves review and truthful recovery controls`,async({page})=>{
    await page.setViewportSize({width:1280,height:640});
    await pendingInstall(page);
    await page.evaluate(code=>window.__failInstall({code,message:'Reviewed fixture failure'}),code);
    const outcome=page.locator('#install-outcome');
    await expect(outcome).toBeVisible();
    await expect(page.locator('#install-outcome-code')).toHaveText(`Result code: ${code}`);
    await expect(page.locator('#install-progress')).toBeHidden();
    await expect(page.locator('#recipe-address')).toBeVisible();
    await expect(page.locator('#install-outcome-recovery')).toBeVisible({visible:code==='rollback_failed'});
    if(code==='rollback_failed') await expect(page.locator('#install-confirm')).toBeDisabled();
    else await expect(page.locator('#install-confirm')).toBeEnabled();
    expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
    expect(await page.evaluate(()=>window.__calls.filter(call=>call.command==='install_app').length)).toBe(1);
    expect(await page.evaluate(()=>window.__calls.some(call=>['engine_setup_action','open_app'].includes(call.command)))).toBe(false);
  });
}
