import {test, expect} from '@playwright/test';
import {installAdapter} from './fixtures.js';
const app = {id:'memos', display_name:'Memos', launch_url:'http://localhost:5230', runtime:{kind:'compose'}, status:'stopped'};
async function defer(page, command) {
  await page.evaluate(command => {
    const original = window.__TAURI__.core.invoke;
    window.__attempts = 0;
    window.__TAURI__.core.invoke = (name, args) => {
      if (name !== command) return original(name, args);
      window.__attempts++;
      return new Promise((resolve, reject) => {
        window.__release = async failure => failure ? reject(failure) : resolve(await original(name, args));
      });
    };
  }, command);
}
test.beforeEach(async ({page}) => { await installAdapter(page, {apps:[app]}); await page.goto('/'); });

test('busy rows preserve keyboard focus and wait for IPC after terminal events', async ({page}) => {
  await page.emulateMedia({reducedMotion:'reduce'});
  await defer(page, 'start_app');
  await page.getByRole('button', {name:/My Apps/}).click();
  const start = page.getByRole('button', {name:'Start', exact:true});
  await start.focus(); await start.press('Enter');
  await expect(start).toBeFocused();
  await expect(start).toHaveAttribute('aria-disabled', 'true');
  await expect(page.getByRole('button', {name:'Logs', exact:true})).toBeEnabled();
  await expect(page.locator('.status')).toHaveText('Starting…');
  await start.press('Enter');
  expect(await page.evaluate(() => window.__attempts)).toBe(1);
  await page.evaluate(() => {
    for (const state of ['started', 'succeeded']) window.__operationListener({payload:{app_id:'memos',kind:'start',operation_id:'run-1',state}});
  });
  await expect(page.locator('.status')).toHaveText('Updating…');
  await expect(start).toHaveAttribute('aria-disabled', 'true');
  await page.evaluate(() => window.__release());
  await expect(page.locator('.status')).toHaveText('running');
  await expect(page.getByRole('button', {name:'Open', exact:true})).toBeFocused();
});

test('busy and error states survive navigation and filtering', async ({page}) => {
  await defer(page, 'start_app');
  await page.getByRole('button', {name:/My Apps/}).click();
  await page.getByRole('button', {name:'Start', exact:true}).click();
  await page.getByRole('button', {name:'Discover', exact:true}).click();
  await page.getByRole('button', {name:/My Apps/}).click();
  await expect(page.locator('.status')).toHaveText('Starting…');
  await expect(page.getByRole('button', {name:'Uninstall Memos'})).toHaveAttribute('aria-disabled', 'true');
  await page.screenshot({path:'.cache/operation-busy.png', fullPage:true});
  await page.evaluate(() => window.__release({code:'timed_out', message:'Docker timed out.'}));
  await expect(page.locator('.inline-error')).toContainText('Check the app status and logs');
  await page.getByPlaceholder('Search your apps…').fill('nothing');
  await expect(page.locator('.installed-app')).toHaveCount(0);
  await page.getByPlaceholder('Search your apps…').fill('');
  await expect(page.locator('.inline-error')).toContainText('Docker timed out.');
  await expect(page.getByRole('button', {name:'Start', exact:true})).not.toHaveAttribute('aria-disabled', 'true');
});

test('install displays honest progress and retains review after failure', async ({page}) => {
  await defer(page, 'install_app');
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  await expect(page.locator('#install-progress')).toBeVisible();
  await expect(page.locator('#install-progress')).toContainText('The local engine may download images');
  await expect(page.getByRole('button', {name:'Installing…', exact:true})).toBeDisabled();
  await page.evaluate(() => {
    const event = {app_id:'memos', kind:'install', operation_id:'install-1'};
    window.__operationListener({payload:{...event, state:'started'}});
    window.__operationListener({payload:{...event, state:'progress', stage:'waiting_for_health'}});
  });
  await expect(page.locator('#install-progress-title')).toHaveText('Waiting for the app to respond…');
  await expect(page.getByRole('button', {name:'Installing…', exact:true})).toBeDisabled();
  await page.screenshot({path:'.cache/operation-install.png', fullPage:true});
  await page.evaluate(() => window.__release({code:'timed_out', message:'Setup timed out.'}));
  await expect(page.locator('#install-error')).toContainText('Check the app status and logs');
  await expect(page.locator('#install-progress')).toBeHidden();
  await expect(page.getByRole('button', {name:'Install Memos', exact:true})).toBeEnabled();
  expect(await page.evaluate(() => window.__attempts)).toBe(1);
});

test('stopping setup is offered only before the commit and never strands focus', async ({page}) => {
  await page.emulateMedia({reducedMotion:'reduce'});
  await defer(page, 'install_app');
  await page.evaluate(() => {
    window.__cancels = [];
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (name, args) => {
      if (name === 'cancel_app_setup') { window.__cancels.push(args); return Promise.resolve(true); }
      return original(name, args);
    };
  });
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  const stop = page.getByRole('button', {name:'Stop setup', exact:true});
  // Nothing to cancel until the backend names the operation.
  await expect(stop).toBeHidden();

  await page.evaluate(() => {
    const event = {app_id:'memos', kind:'install', operation_id:'install-1'};
    window.__operationListener({payload:{...event, state:'started', cancel_id:42}});
    window.__operationListener({payload:{...event, state:'progress', stage:'starting_containers'}});
  });
  await expect(stop).toBeVisible();

  // Focus the control, then push the install past the commit cutoff: the
  // control must go away and focus must land somewhere real.
  await stop.focus();
  await page.evaluate(() => window.__operationListener({payload:{app_id:'memos', kind:'install', operation_id:'install-1', state:'progress', stage:'saving_app'}}));
  await expect(stop).toBeHidden();
  await expect(page.locator('#install-progress')).toBeFocused();
  expect(await page.evaluate(() => window.__cancels.length)).toBe(0);

  await page.evaluate(() => window.__release({code:'timed_out', message:'Setup timed out.'}));
  await expect(page.locator('#install-progress')).toBeHidden();
});

test('stopping setup sends the operation id and waits for IPC to settle', async ({page}) => {
  await page.emulateMedia({reducedMotion:'reduce'});
  await defer(page, 'install_app');
  await page.evaluate(() => {
    window.__cancels = [];
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (name, args) => {
      if (name === 'cancel_app_setup') { window.__cancels.push(args); return Promise.resolve(true); }
      return original(name, args);
    };
  });
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  await page.evaluate(() => window.__operationListener({payload:{app_id:'memos', kind:'install', operation_id:'install-1', state:'started', cancel_id:99}}));
  const stop = page.getByRole('button', {name:'Stop setup', exact:true});
  await stop.click();

  // The request names both app and operation, so it cannot hit a successor.
  expect(await page.evaluate(() => window.__cancels)).toEqual([{id:'memos', operationId:99}]);
  // One stop only, and the install is still running until IPC settles it.
  await expect(stop).toBeHidden();
  await expect(page.locator('#install-progress')).toBeVisible();
  await expect(page.getByRole('button', {name:'Installing…', exact:true})).toBeDisabled();

  await page.evaluate(() => window.__release({code:'cancelled', message:'Setup was cancelled before it finished.'}));
  await expect(page.locator('#install-error')).toContainText('Setup was cancelled');
  await expect(page.getByRole('button', {name:'Install Memos', exact:true})).toBeEnabled();
  expect(await page.evaluate(() => window.__attempts)).toBe(1);
});

test('readiness refines a running row without inventing a state it did not check', async ({page}) => {
  await page.evaluate(() => {
    window.__readiness = {};
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (name, args) =>
      name === 'app_readiness' ? Promise.resolve(window.__readiness[args.id]) : original(name, args);
  });
  await page.getByRole('button', {name:'My Apps'}).click();
  const status = page.locator('.installed-app[data-app-id="memos"] .status');
  // An unanswered or unknown readiness must leave the container status alone.
  await expect(status).toHaveText('stopped');

  await page.evaluate(() => { window.__readiness = {memos: 'ready'}; });
  await page.getByRole('button', {name:'Start', exact:true}).click();
  await expect(status).toHaveText('ready');
  await expect(status).not.toHaveClass(/status-unreachable/);

  // A running container that is not answering says so, and says it plainly.
  await page.evaluate(() => { window.__readiness = {memos: 'unreachable'}; });
  await page.getByRole('button', {name:'My Apps'}).click();
  await expect(status).toHaveText('not responding');
  await expect(status).toHaveClass(/status-unreachable/);

  // An unknown answer falls back to the container status rather than sticking.
  await page.evaluate(() => { window.__readiness = {memos: 'unknown'}; });
  await page.getByRole('button', {name:'My Apps'}).click();
  await expect(status).toHaveText('running');
});

test('a readiness failure never disturbs the row', async ({page}) => {
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (name, args) =>
      name === 'app_readiness' ? Promise.reject({code:'process_unavailable', message:'no probe'}) : original(name, args);
  });
  await page.getByRole('button', {name:'My Apps'}).click();
  const row = page.locator('.installed-app[data-app-id="memos"]');
  await expect(row.locator('.status')).toHaveText('stopped');
  await expect(row.locator('.inline-error')).toHaveText('');
});

test('a link the browser refused is reported where the user can see it', async ({page}) => {
  // The failure happens inside an app window, which owns none of the launcher
  // UI, so the launcher is what has to say something.
  await page.evaluate(() => window.__browserFailureListener({payload: {code: 'browser_open_failed', message: 'Could not open your browser: no handler.'}}));
  const toast = page.locator('#toast');
  await expect(toast).toBeVisible();
  await expect(toast).toContainText('Could not open your browser');
});

test('a malformed browser failure payload is ignored', async ({page}) => {
  await page.evaluate(() => {
    window.__browserFailureListener({payload: null});
    window.__browserFailureListener({payload: {code: 'browser_open_failed'}});
  });
  await expect(page.locator('#toast')).toBeHidden();
});

test('native activation failures are drained and shown by the launcher', async ({page}) => {
  await page.evaluate(async () => {
    const original = window.__TAURI__.core.invoke;
    let pending = [{code:'window_open_failed', message:'Could not open the requested app.'}];
    window.__TAURI__.core.invoke = (name, args) => {
      if (name !== 'take_activation_errors') return original(name, args);
      const errors = pending; pending = []; return Promise.resolve(errors);
    };
    await window.__activationFailureListener({payload:null});
    await window.__activationFailureListener({payload:null});
  });
  await expect(page.locator('#toast')).toBeVisible();
  await expect(page.locator('#toast')).toContainText('Could not open the requested app.');
});

test('checking an address reports what was found and never blocks saving', async ({page}) => {
  await page.evaluate(() => {
    window.__checks = [];
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (name, args) => {
      if (name === 'check_address') { window.__checks.push(args.url); return Promise.resolve(window.__addressResult); }
      return original(name, args);
    };
  });
  await page.getByRole('button', {name:'Connect an app', exact:true}).first().click();
  const status = page.locator('#connect-check');
  const check = page.getByRole('button', {name:'Check address', exact:true});

  // Nothing typed yet: say so rather than probing an empty address.
  await check.click();
  await expect(status).toHaveText('Enter an address to check.');
  expect(await page.evaluate(() => window.__checks.length)).toBe(0);

  await page.getByLabel('Instance address').fill('http://localhost:9999');
  await page.evaluate(() => { window.__addressResult = 'unreachable'; });
  await check.click();
  await expect(status).toContainText('No answer at that address');
  // An app that is merely switched off is still worth saving.
  await expect(page.locator('#connect-submit')).toBeEnabled();
  await expect(page.locator('#connect-error')).toHaveText('');

  await page.evaluate(() => { window.__addressResult = 'ready'; });
  await check.click();
  await expect(status).toContainText('That address answered');

  // An address this build cannot probe must not be reported as a problem.
  await page.evaluate(() => { window.__addressResult = 'unknown'; });
  await check.click();
  await expect(status).toContainText('cannot be checked from here');
  expect(await page.evaluate(() => window.__checks)).toEqual(
    ['http://localhost:9999', 'http://localhost:9999', 'http://localhost:9999']);

  // Editing the address clears a stale verdict.
  await page.getByLabel('Instance address').fill('http://localhost:5230');
  await expect(status).toHaveText('');
});

test('a rejected address reports the reason against the field', async ({page}) => {
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (name, args) =>
      name === 'check_address'
        ? Promise.reject({code:'invalid_input', message:'Enter a complete http:// or https:// address.'})
        : original(name, args);
  });
  await page.getByRole('button', {name:'Connect an app', exact:true}).first().click();
  await page.getByLabel('Instance address').fill('ftp://example.com');
  await page.getByRole('button', {name:'Check address', exact:true}).click();
  await expect(page.locator('#connect-error')).toContainText('http:// or https://');
  await expect(page.locator('#connect-check')).toHaveText('');
  await expect(page.getByLabel('Instance address')).toHaveAttribute('aria-invalid', 'true');
});

test('a failed cleanup blocks a blind retry until the user has looked', async ({page}) => {
  await defer(page, 'install_app');
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  await page.evaluate(() => window.__release({
    code: 'rollback_failed',
    message: 'Start failed. Cleanup also failed: Compose down refused.',
  }));
  await expect(page.locator('#install-error')).toContainText('containers or files may remain');
  const confirm = page.locator('#install-confirm');
  await expect(confirm).toBeDisabled();
  await expect(confirm).toHaveText('Review needed before retrying');

  // Reopening the review is the deliberate way back, and it clears the block.
  await page.getByRole('button', {name:'Back to Discover'}).click();
  await page.locator('[data-featured="memos"]').click();
  await expect(page.getByRole('button', {name:'Install Memos', exact:true})).toBeEnabled();
});

test('an ordinary failure still offers a retry, because it rolled back cleanly', async ({page}) => {
  await defer(page, 'install_app');
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  await page.evaluate(() => window.__release({code:'port_in_use', message:'Port 5230 is already in use.'}));
  await expect(page.locator('#install-error')).toContainText('Choose an unused port');
  await expect(page.getByRole('button', {name:'Install Memos', exact:true})).toBeEnabled();
});

test('deleting app data needs the checkbox and the confirmation, and dismissing it deletes nothing', async ({page}) => {
  await page.getByRole('button', {name:/My Apps/}).click();
  await page.getByRole('button', {name:'Uninstall Memos'}).click();
  await expect(page.getByRole('button', {name:'Uninstall, keep data', exact:true})).toBeVisible();

  // The destructive wording only appears once data deletion is chosen.
  await page.getByLabel(/Delete app data too/).check();
  const destructive = page.getByRole('button', {name:'Uninstall and delete data', exact:true});
  await expect(destructive).toBeVisible();

  // A wrong name and backing out must delete nothing at all.
  await expect(destructive).toBeDisabled();
  await page.locator('#delete-name').fill('memos');
  await expect(destructive).toBeDisabled();
  await page.getByRole('button', {name:'Keep app', exact:true}).click();
  expect(await page.evaluate(() => window.__calls.filter(c => c.command === 'uninstall_app'))).toEqual([]);

  await page.getByRole('button', {name:'Uninstall Memos'}).click();
  await page.getByLabel(/Delete app data too/).check();
  await page.locator('#delete-name').fill('Memos');
  await destructive.click();
  await expect(page.getByRole('heading', {name:'Your apps belong here.'})).toBeVisible();
  expect(await page.evaluate(() => window.__calls.find(c => c.command === 'uninstall_app')))
    .toEqual({command:'uninstall_app', args:{id:'memos', deleteData:true}});
});

test('the delete-data choice does not persist into the next uninstall', async ({page}) => {
  await page.getByRole('button', {name:/My Apps/}).click();
  await page.getByRole('button', {name:'Uninstall Memos'}).click();
  await page.getByLabel(/Delete app data too/).check();
  await page.getByRole('button', {name:'Keep app', exact:true}).click();
  await page.getByRole('button', {name:'Uninstall Memos'}).click();
  // A destructive choice must never be inherited by a later, separate decision.
  await expect(page.getByLabel(/Delete app data too/)).not.toBeChecked();
  await expect(page.getByRole('button', {name:'Uninstall, keep data', exact:true})).toBeVisible();
});

test('the pinned port is used unless a different one is deliberately chosen', async ({page}) => {
  await page.locator('[data-featured="memos"]').click();
  // The choice is offered but not imposed; the field stays out of the way.
  await expect(page.locator('#port-field')).toBeHidden();
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  await expect(page.getByRole('heading', {name:/Right where you left them/})).toBeVisible();
  // An untouched review sends exactly what it always sent.
  expect(await page.evaluate(() => window.__calls.find(c => c.command === 'install_app')))
    .toEqual({command:'install_app', args:{recipeId:'memos'}});
});

test('choosing a port updates the stated address and is sent with the install', async ({page}) => {
  await page.locator('[data-featured="memos"]').click();
  await expect(page.locator('#recipe-address')).toHaveText('http://localhost:5230');
  await page.getByRole('button', {name:'Use a different port', exact:true}).click();
  const port = page.locator('#recipe-port');
  await expect(port).toBeFocused();
  // The container port is stated, so it is clear what is and is not moving.
  await expect(page.locator('#port-help')).toContainText('5230 inside its container');

  await port.fill('8080');
  await expect(page.locator('#recipe-address')).toHaveText('http://localhost:8080');
  await page.getByRole('button', {name:'Install Memos', exact:true}).click();
  await expect(page.getByRole('heading', {name:/Right where you left them/})).toBeVisible();
  expect(await page.evaluate(() => window.__calls.find(c => c.command === 'install_app')))
    .toEqual({command:'install_app', args:{recipeId:'memos', hostPort:8080}});
});

test('an unusable port is refused before Docker is asked to do anything', async ({page}) => {
  await page.locator('[data-featured="memos"]').click();
  await page.getByRole('button', {name:'Use a different port', exact:true}).click();
  for (const value of ['80', '0', '99999', '']) {
    await page.locator('#recipe-port').fill(value);
    await page.getByRole('button', {name:'Install Memos', exact:true}).click();
    await expect(page.locator('#install-error')).toContainText('between 1024 and 65535');
    await expect(page.locator('#recipe-port')).toHaveAttribute('aria-invalid', 'true');
    expect(await page.evaluate(() => window.__calls.filter(c => c.command === 'install_app'))).toEqual([]);
  }
  // A usable port clears the objection.
  await page.locator('#recipe-port').fill('9000');
  await expect(page.locator('#install-error')).toHaveText('');
});
