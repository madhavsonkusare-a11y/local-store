import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';
async function setup(page,{unavailable=false}={}) {
  await installAdapter(page);
  await page.addInitScript(({unavailable})=>{
    const original=window.__TAURI__.core.invoke;
    let clients=[],grants=[];
    window.__TAURI__.core.invoke=async(command,args={})=>{
      if(!command.startsWith('agent_'))return original(command,args);
      window.__calls.push({command,args});
      if(command==='agent_connections')return {clients,apps:[{id:'memos',display_name:'Memos',managed:true}],grants,audit:[{client_id:'old-agent',app_id:'memos',action:'status',allowed:false,at_unix:1}],sidecar_available:!unavailable,same_user_limitation:'Permissions are not a sandbox for the Windows account.'};
      if(command==='agent_mutation_requests')return [];
      if(command==='agent_enrollment_begin'){clients.push(args.clientId);return {enrollment_id:'preview-id',client_id:args.clientId};}
      if(command==='agent_enrollment_export'){if(!args.consent)throw new Error('Confirm this connection before continuing.');return {configuration:{mcpServers:{'local-store':{command:'D:/Local Store/local-store-mcp.exe',env:{LOCAL_STORE_AGENT_BEARER:'test-only-private-token'}}}}};}
      if(command==='agent_enrollment_cancel'||command==='agent_client_revoke'){clients=[];grants=[];return;}
      if(command==='agent_grant_set'){if(!args.consent)throw new Error('Consent required');grants=[{client_id:args.clientId,app_id:args.appId,actions:args.scope==='lifecycle'?['status','start','stop']:['status'],expires_at_unix:9999999999,expired:false,installed:true}];return 9999999999;}
      if(command==='agent_grant_revoke'){grants=[];return;}
    };
  },{unavailable});
  await page.goto('/');await page.locator('#settings').click();await page.locator('#agent-refresh').click();
}
test('agent connection requires consent, reveals configuration once and clears it on close',async({page})=>{
  await setup(page);
  await page.getByLabel('Agent name',{exact:true}).fill('hermes');
  await page.getByRole('button',{name:'Create connection',exact:true}).click();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='agent_enrollment_begin'))).toBe(false);
  await page.locator('#agent-enroll-consent').check();await page.getByRole('button',{name:'Create connection',exact:true}).click();
  await expect(page.locator('#agent-preview')).toContainText('has no app access');
  await expect(page.locator('#agent-configuration')).toBeHidden();
  await page.getByRole('button',{name:'Show configuration',exact:true}).click();
  await expect(page.locator('#agent-error')).toContainText('Confirm this connection');
  await page.locator('#agent-export-consent').check();await page.getByRole('button',{name:'Show configuration',exact:true}).click();
  await expect(page.locator('#agent-configuration')).toHaveValue(/test-only-private-token/);
  await expect(page.locator('#agent-export')).toBeHidden();
  expect(await page.evaluate(()=>Object.values(localStorage).join('').includes('test-only-private-token'))).toBe(false);
  await page.keyboard.press('Escape');await expect(page.locator('#settings-dialog')).toBeHidden();
  await expect(page.locator('#agent-configuration')).toHaveValue('');
});
test('app permission expiry, audit and reviewed removal use exact scopes',async({page})=>{
  await setup(page);await page.locator('#agent-client-name').fill('hermes');await page.locator('#agent-enroll-consent').check();await page.getByRole('button',{name:'Create connection',exact:true}).click();
  await page.locator('#agent-grant-scope').selectOption('lifecycle');await expect(page.locator('#agent-grant-hours')).toHaveAttribute('max','24');
  await page.locator('#agent-grant-hours').fill('2');await page.locator('#agent-grant-consent').check();await page.locator('#agent-grant-submit').click();
  await expect(page.locator('#agent-grants')).toContainText('status, start, stop');
  const call=await page.evaluate(()=>window.__calls.find(call=>call.command==='agent_grant_set'));
  expect(call.args).toEqual({clientId:'hermes',appId:'memos',scope:'lifecycle',hours:2,consent:true});
  await expect(page.locator('#agent-audit')).toContainText('denied');
  await page.getByRole('button',{name:'Review removal',exact:true}).click();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='agent_client_revoke'))).toBe(false);
  await page.getByRole('button',{name:'Remove connection',exact:true}).click();
  await expect(page.locator('#agent-clients')).toHaveText('');await expect(page.locator('#agent-grant-submit')).toBeDisabled();
});
test('missing connector disables enrollment and management screen remains accessible',async({page})=>{
  await setup(page,{unavailable:true});await expect(page.getByRole('button',{name:'Create connection',exact:true})).toBeDisabled();
  await expect(page.locator('#agent-status')).toContainText('not installed');
  await expect(page.locator('#agent-panel')).toContainText('Manage Memos content permissions separately below');
  const violations=(await new AxeBuilder({page}).include('#settings-dialog').analyze()).violations;
  expect(violations).toEqual([]);
});

test('pending install requests require a separate explicit one-use approval',async({page})=>{
  await setup(page);
  await page.evaluate(()=>{
    const original=window.__TAURI__.core.invoke;
    let state='pending';
    window.__TAURI__.core.invoke=(command,args)=>{
      if(command==='agent_mutation_requests')return Promise.resolve([{id:'review-id',client_id:'hermes',target:{kind:'install',display_name:'Memos'},state,approved_until_unix:9999999999}]);
      if(command==='agent_mutation_decide'){window.__calls.push({command,args});state=args.approve?'approved':'denied';return Promise.resolve({state});}
      return original(command,args);
    };
  });
  await page.locator('#agent-refresh').click();
  await expect(page.getByRole('button',{name:'Approve once',exact:true})).toBeDisabled();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='agent_mutation_decide'))).toBe(false);
  await page.getByLabel('Allow hermes to install this app once.',{exact:true}).check();
  await page.getByRole('button',{name:'Approve once',exact:true}).click();
  await expect(page.locator('#agent-requests')).toContainText('Approved until');
  const call=await page.evaluate(()=>window.__calls.find(call=>call.command==='agent_mutation_decide'));
  expect(call.args).toEqual({requestId:'review-id',approve:true,consent:true});
});
