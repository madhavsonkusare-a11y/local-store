import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

for(const state of ['loading','empty','failure']) test(`selected app logs ${state} preserve identity and accessible recovery`,async({page})=>{
  await page.setViewportSize({width:1280,height:640});
  await installAdapter(page,{apps:[{id:'memos',display_name:'Memos',launch_url:'http://localhost:5230',runtime:{kind:'compose'},status:'running'}]});
  await page.addInitScript(state=>{
    const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='app_logs') {
        window.__calls.push({command,args});
        if(state==='empty') return Promise.resolve('');
        if(state==='failure') return Promise.reject({code:'timed_out',message:'Could not read current output'});
        return new Promise(resolve=>{window.__finishLogs=resolve;});
      }
      return invoke(command,args);
    };
  },state);
  await page.goto('/#my-apps');
  await page.getByRole('tab',{name:'Logs',exact:true}).click();
  await expect(page.locator('#my-apps-detail-title')).toHaveText('Memos');
  const panel=page.locator('#my-apps-panel');
  if(state==='loading') {
    await expect(panel.getByRole('button',{name:'Loading…',exact:true})).toBeDisabled();
    await expect(panel).toContainText('Loading recent output…');
  } else if(state==='empty') await expect(panel.locator('.log-view')).toHaveText('No recent output.');
  else {
    await expect(panel.getByRole('alert')).toContainText('Could not read current output');
    await expect(panel.getByRole('button',{name:'Refresh logs',exact:true})).toBeEnabled();
    await expect(panel.locator('.log-view')).toHaveCount(0);
  }
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  expect(await page.evaluate(()=>window.__calls.filter(call=>call.command==='app_logs'))).toEqual([{command:'app_logs',args:{id:'memos'}}]);
  if(state==='loading') {
    await page.getByRole('tab',{name:'Manage',exact:true}).click();
    await page.evaluate(()=>window.__finishLogs('Late reviewed output'));
    await expect(page.getByRole('tab',{name:'Manage',exact:true})).toHaveAttribute('aria-selected','true');
    await expect(panel.locator('.log-view')).toHaveCount(0);
  }
});
