import {test, expect} from '@playwright/test';
import {installAdapter} from './fixtures.js';

test('recovery scan supports empty, verified, error and late replies', async ({page}) => {
  await page.emulateMedia({reducedMotion:'reduce'});
  await installAdapter(page);
  await page.goto('/');
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__recoveryAnswer = [];
    window.__TAURI__.core.invoke = (name, args) => {
      if (name !== 'inspect_recovery') return original(name, args);
      if (window.__recoveryAnswer === 'error') return Promise.reject(new Error('Docker is unavailable'));
      if (window.__recoveryAnswer === 'pending') return new Promise(resolve => { window.__resolveRecovery = resolve; });
      return Promise.resolve(window.__recoveryAnswer);
    };
  });
  await page.locator('#settings').click();
  const scan = page.getByRole('button', {name:'Scan for setups'});
  await scan.click();
  await expect(page.locator('#recovery-output')).toHaveText('No retained setups found for supported apps.');
  await page.evaluate(() => { window.__recoveryAnswer = [{display_name:'<img onerror=alert(1)>', ownership_status:'verified', compose_file:'D:/private/compose.yaml'}]; });
  await scan.click();
  await expect(page.locator('#recovery-output')).toContainText('Matching Docker setup found');
  await expect(page.locator('#recovery-output img')).toHaveCount(0);
  await page.getByText('Setup location', {exact:true}).click();
  await expect(page.locator('#recovery-output')).toContainText('D:/private/compose.yaml');
  await page.evaluate(() => { window.__recoveryAnswer = 'error'; });
  await scan.click();
  await expect(page.locator('#recovery-error')).toHaveText('Docker is unavailable');
  await expect(scan).toBeEnabled();
  await page.evaluate(() => { window.__recoveryAnswer = 'pending'; });
  await scan.click();
  await expect(page.getByRole('button', {name:'Scanning…'})).toBeDisabled();
  await page.keyboard.press('Escape');
  await page.evaluate(() => window.__resolveRecovery([{display_name:'Stale', ownership_status:'verified'}]));
  await page.locator('#settings').click();
  await expect(page.locator('#recovery-output')).not.toContainText('Stale');
  await expect(scan).toBeEnabled();
});


test('verified recovery rechecks backend ownership and confirms data deletion explicitly', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=> {
      if(command==='inspect_recovery') return Promise.resolve([{recipe_id:'memos',display_name:'Memos',ownership_status:'verified',docker_ownership_verified:true,compose_file:'D:/retained/memos/compose.yaml'}]);
      if(command==='discard_retained_setup') {window.__calls.push({command,args}); return Promise.resolve({recipe_id:'memos',containers_removed:1,data_deleted:args.deleteData});}
      return original(command,args);
    };
  });
  await page.goto('/'); await page.locator('#settings').click(); await page.locator('#scan-recovery').click();
  await page.getByRole('button',{name:'Review setup',exact:true}).click();
  await expect(page.locator('#recovery-task')).toBeVisible();
  await page.locator('#recovery-delete-data').check();
  await expect(page.locator('#recovery-clear')).toBeDisabled();
  await page.locator('#recovery-delete-confirm').fill('memos');
  await expect(page.locator('#recovery-clear')).toBeDisabled();
  expect(await page.evaluate(()=>window.__calls.some(call=>call.command==='discard_retained_setup'))).toBe(false);
  await page.locator('#recovery-delete-confirm').fill('Memos'); await page.locator('#recovery-clear').click();
  await expect(page.locator('#recovery-task-status')).toHaveText('Retained setup and managed data deleted.');
  expect(await page.evaluate(()=>window.__calls.find(call=>call.command==='discard_retained_setup').args)).toEqual({recipeId:'memos',deleteData:true});
  await page.keyboard.press('Escape'); await expect(page.locator('#settings-dialog')).toBeVisible();
});

test('uncertain setup ownership never offers a mutation and recovery failure never reports success', async ({page}) => {
  await installAdapter(page);
  await page.addInitScript(() => {
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=> {
      if(command==='inspect_recovery') return Promise.resolve([{recipe_id:'memos',display_name:'Memos',ownership_status:'mismatch',docker_ownership_verified:false,compose_file:'D:/retained/memos/compose.yaml'}]);
      return original(command,args);
    };
  });
  await page.goto('/'); await page.locator('#settings').click(); await page.locator('#scan-recovery').click();
  await expect(page.getByRole('button',{name:'Review setup',exact:true})).toHaveCount(0);
  await expect(page.locator('#recovery-output')).toContainText('could not be verified');
  await page.evaluate(() => {
    const original=window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke=(command,args)=> {
      if(command==='inspect_recovery') return Promise.resolve([{recipe_id:'memos',display_name:'Memos',ownership_status:'verified',docker_ownership_verified:true,compose_file:'D:/retained/memos/compose.yaml'}]);
      if(command==='adopt_retained_setup') return new Promise((resolve,reject)=>{window.__rejectRecovery=reject;});
      return original(command,args);
    };
  });
  await page.locator('#scan-recovery').click(); await page.getByRole('button',{name:'Review setup',exact:true}).click();
  await page.locator('#recovery-resume').click();
  await expect(page.locator('#recovery-task-status')).toContainText('Checking ownership');
  await page.keyboard.press('Escape'); await expect(page.locator('#recovery-task')).toBeVisible();
  await page.evaluate(()=>window.__rejectRecovery(new Error('Ownership changed; recovery refused')));
  await expect(page.locator('#recovery-task-error')).toContainText('Ownership changed');
  await expect(page.locator('#recovery-task-status')).toHaveText('Recovery did not finish. No success is assumed.');
  await expect(page.locator('#recovery-resume')).toBeEnabled();
});
