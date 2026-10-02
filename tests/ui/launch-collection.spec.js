import {test,expect} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {installAdapter} from './fixtures.js';
const selected = JSON.parse(readFileSync(new URL('../../catalog/v1-qualified-apps.json',import.meta.url),'utf8')).apps.map(app=>app.id);
test('launch counts use every selected app and require current task proof',async({page})=>{
  await installAdapter(page);
  await page.addInitScript(ids=>{
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='launch_readiness_batch' && args.ids.length===10){
        window.__launchIds=args.ids;
        return Promise.resolve(ids.map((id,index)=>({offering_id:id,install_mode:index<3?'setup_assisted':'zero_input',task_verified:true,current_evidence:index!==9})));
      }
      return original(command,args);
    };
  },selected);
  await page.goto('/');await page.locator('#settings').click();
  await expect(page.locator('#launch-collection-summary')).toHaveText('10 selected apps · 7 zero-input installs · 3 setup-assisted installs · 9 current verified tasks');
  expect((await page.evaluate(()=>window.__launchIds)).sort()).toEqual([...selected].sort());
});
test('missing proof never shows a completed launch count',async({page})=>{
  await installAdapter(page);await page.goto('/');await page.locator('#settings').click();
  await expect(page.locator('#launch-collection-summary')).toContainText('No verified count is available');
});
