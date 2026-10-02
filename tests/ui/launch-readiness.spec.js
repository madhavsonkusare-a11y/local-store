import {test, expect} from '@playwright/test';
import {installAdapter} from './fixtures.js';
const proof = {offering_id:'memos',install_mode:'zero_input',lifecycle_proven:true,task_verified:true,current_evidence:true,task:'An exact note survives restart and keep-data reinstall.',agent_content_access:'unverified',prerequisites:['Create the first app account after opening.'],caution:null};

test('provider capability distinguishes read-only from writes without granting access', async ({page}) => {
  await installAdapter(page);
  await page.goto('/');
  const wording = await page.evaluate(async () => {
    const {readinessView} = await import('/js/launch-readiness.js');
    return ['verified_read','verified_read_write','unverified'].map(agent_content_access => readinessView({agent_content_access,install_mode:'zero_input'}));
  });
  expect(wording[0]).toContain('Read-only provider verified · separate permission required');
  expect(wording[1]).toContain('each write needs approval');
  expect(wording[2]).toContain('App content access is not verified');
  expect(await page.evaluate(() => window.__calls.some(call => call.command.includes('grant') || call.command.includes('content_connect')))).toBe(false);
});

test('current task proof and unverified content access stay distinct before downloading', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(proof => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => {
      if (command === 'launch_readiness_batch') return Promise.resolve([proof]);
      if (command === 'launch_readiness') return Promise.resolve(proof);
      return original(command,args);
    };
  },proof);
  await page.goto('/');
  await expect(page.locator('.app-card').filter({hasText:'Memos'})).toContainText('Task verified · Windows');
  await page.locator('[data-featured="memos"]').click();
  await expect(page.locator('#install-dialog')).toBeVisible();
  await expect(page.locator('#install-content')).toContainText('An exact note survives restart');
  await expect(page.locator('#install-content')).toContainText('App content access is not verified');
  await expect(page.locator('#install-content')).toContainText('Before downloading');
  await expect(page.locator('#install-content')).toContainText('Create the first app account');
  expect(await page.evaluate(() => window.__calls.some(call => call.command === 'install_app'))).toBe(false);
  await page.keyboard.press('Escape');
  await expect(page.locator('#install-dialog')).not.toBeVisible();
});

test('stale proof never becomes a current task badge', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(proof => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => command === 'launch_readiness_batch' ? Promise.resolve([{...proof,current_evidence:false,caution:'Reverification required'}]) : original(command,args);
  },proof);
  await page.goto('/');
  await expect(page.locator('.app-card').filter({hasText:'Memos'})).toContainText('Install preview');
  await expect(page.locator('.app-card').filter({hasText:'Memos'})).not.toContainText('Task verified');
});

test('first run only records viewed explanations and never grants engine or agent consent', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args) => {
      if(command === 'onboarding_progress') {window.__calls.push({command,args}); return Promise.resolve({viewed:[]});}
      if(command === 'mark_onboarding_viewed') {window.__calls.push({command,args}); return Promise.resolve({viewed:[args.step]});}
      return original(command,args);
    };
  });
  await page.goto('/');
  await expect(page.locator('#first-run-dialog')).toBeVisible();
  await page.getByRole('button',{name:'Explore apps',exact:true}).click();
  await expect(page.locator('#first-run-dialog')).not.toBeVisible();
  const calls=await page.evaluate(()=>window.__calls);
  expect(calls.filter(call=>call.command==='mark_onboarding_viewed')).toHaveLength(4);
  expect(calls.some(call=>call.command==='engine_setup_action'||call.command.includes('agent_grant'))).toBe(false);
});

test('focused install retains commit controls at compact height', async ({page}) => {
  await page.setViewportSize({width:800,height:600});
  await installAdapter(page);
  await page.goto('/');
  await page.locator('[data-featured="memos"]').click();
  await expect(page.locator('#install-confirm')).toBeVisible();
  expect(await page.locator('#install-dialog').evaluate(dialog => getComputedStyle(dialog,'::backdrop').backdropFilter)).toBe('none');
  const bounds=await page.locator('#install-confirm').boundingBox();
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(600);
});


test('engine setup requires all explicit choices and never falls back to Docker Desktop', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    const engine={bootstrap:{status:'not_configured'},prerequisites:{state:'ready'},daemon:'not_checked'};
    window.__TAURI__.core.invoke=(command,args)=> {
      if(command==='engine_setup_preview') return Promise.resolve({engine,payload:'verified_development',required_disk_bytes:2248767488,disk_sufficient:true,payload_release_approved:false,selected_engine:null,development_engine_available:false});
      if(command==='engine_setup_action') {window.__calls.push({command,args}); return Promise.reject(new Error('Payload changed; setup refused'));}
      return original(command,args);
    };
  });
  await page.goto('/'); await page.locator('#settings').click();
  await expect(page.locator('#engine-output')).toContainText('Docker Desktop is not required');
  await page.locator('#engine-setup-start').click();
  await expect(page.locator('#engine-error')).toContainText('accept each engine setup choice');
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='engine_setup_action'))).toBe(false);
  for(const checkbox of await page.locator('[data-engine-consent]').all()) await checkbox.check();
  await page.locator('#engine-setup-start').click();
  await expect(page.locator('#engine-error')).toContainText('Payload changed; setup refused');
  await expect(page.locator('#engine-setup-start')).toBeEnabled();
  const call=await page.evaluate(()=>window.__calls.find(call=>call.command==='engine_setup_action'));
  expect(call.args).toEqual({action:'install',consent:{create_owned_engine:true,use_disk_space:true,acknowledge_existing_apps_unchanged:true}});
});

test('missing bundled payload blocks setup instead of suggesting another engine', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=> command==='engine_setup_preview' ? Promise.resolve({engine:{bootstrap:{status:'not_configured'},prerequisites:{state:'ready'},daemon:'not_checked'},payload:'missing',required_disk_bytes:2248767488,disk_sufficient:true,development_engine_available:false}) : original(command,args);
  });
  await page.goto('/'); await page.locator('#settings').click();
  await expect(page.locator('#engine-output')).toContainText('Managed installation is blocked');
  await expect(page.locator('#engine-setup-start')).toHaveCount(0);
});
