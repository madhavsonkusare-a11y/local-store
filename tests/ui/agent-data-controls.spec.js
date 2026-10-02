import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';
test('note-file access needs exact app/client consent, exposes no token and is accessible',async({page})=>{
  await installAdapter(page);await page.addInitScript(()=>{const original=window.__TAURI__.core.invoke;window.__TAURI__.core.invoke=async(command,args={})=>{if(command==='agent_connections'){window.__calls.push({command,args});return{clients:['hermes'],apps:[{id:'flatnotes',display_name:'My notes',managed:true},{id:'external-notes',display_name:'Linked notes',managed:false}],grants:[],audit:[]};}if(command==='agent_data_grant'){window.__calls.push({command,args});return 9999999999;}return original(command,args);};});
  await page.goto('/');await page.locator('#settings').click();await page.locator('#note-files-refresh').click();
  await expect(page.getByLabel('Installed Flatnotes app')).toContainText('My notes');await expect(page.getByLabel('Installed Flatnotes app')).not.toContainText('Linked notes');
  await expect(page.locator('#note-files-form')).toContainText('No file writes');
  await page.locator('#note-files-save').click();expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='agent_data_grant'))).toBe(false);
  await page.locator('#note-files-consent').check();await page.locator('#note-files-save').click();
  expect(await page.evaluate(()=>window.__calls.find(call=>call.command==='agent_data_grant'))).toEqual({command:'agent_data_grant',args:{clientId:'hermes',appId:'flatnotes',hours:1,consent:true}});
  await expect(page.locator('#note-files-consent')).not.toBeChecked();expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
});
