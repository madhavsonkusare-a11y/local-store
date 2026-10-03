import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { installAdapter } from './fixtures.js';

test.beforeEach(async ({page}) => { await installAdapter(page); await page.goto('/'); });
test('discover search, category, detail and source actions are honest', async ({page}) => {
 await expect(page.getByRole('heading',{name:'Good software. Your space.'})).toBeVisible();
 await expect(page.getByText(/5 projects to discover/)).toBeVisible();
 await page.getByRole('searchbox').fill('memo'); await expect(page.getByRole('heading',{name:'Memos'})).toBeVisible();
 await expect(page.getByRole('heading',{name:'Immich'})).toHaveCount(0);
 await page.getByRole('searchbox').fill('');
 await page.getByRole('button',{name:'Filters',exact:true}).click();
 await page.getByLabel('Find a category').fill('Photo');
 await page.getByRole('button',{name:/Photo Galleries/}).click();
 await page.getByRole('button',{name:'Show results'}).click();
 await expect(page.getByRole('heading',{name:'Immich'})).toBeVisible();
 await page.getByRole('button',{name:'View Immich details'}).click();
 await expect(page.getByText('Connect existing instance')).toBeVisible();
 await expect(page.getByText(/Automatic installation is available only/)).toBeVisible();
 await page.getByRole('button',{name:/Visit project website/}).click();
 expect(await page.evaluate(() => window.__calls.at(-1))).toEqual({command:'open_project',args:{url:'https://immich.app'}});
});

test('reviewed Memos recipe shows prerequisites and installs into My Apps', async ({page}) => {
 await page.getByRole('searchbox').fill('memo'); await page.getByRole('button',{name:'View Memos details'}).click();
 await expect(page.getByText('Preview local install')).toBeVisible(); await page.locator('#detail-dialog').getByRole('button',{name:'Review install'}).click();
 await expect(page.getByText('neosmemo/memos:0.30.0')).toBeVisible(); await expect(page.getByText('Docker engine')).toBeVisible();
 await page.getByRole('button',{name:'Install Memos'}).click();
 await expect(page.locator('#install-dialog')).not.toBeVisible();
 await expect(page.getByRole('dialog',{name:'Memos installed'})).toBeVisible();
 await page.getByRole('button',{name:'Enter workspace',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Memos',exact:true})).toBeVisible();
 expect(await page.evaluate(() => window.__calls.find(c=>c.command==='install_app'))).toEqual({command:'install_app',args:{recipeId:'memos'}});
});

test('all graduated recipes remain explicitly reviewed before install', async ({page}) => {
 for (const [query, name, image] of [['n8n','n8n','docker.n8n.io/n8nio/n8n:2.37.10'],['uptime','Uptime Kuma','louislam/uptime-kuma:2.5.3']]) {
   await page.getByRole('searchbox').fill(query);
   await expect(page.getByRole('heading',{name})).toBeVisible();
   await page.getByRole('button',{name:`View ${name} details`}).click();
   await expect(page.getByText('Preview local install')).toBeVisible();
   await page.locator('#detail-dialog').getByRole('button',{name:'Review install'}).click();
   await expect(page.getByText(image)).toBeVisible();
   await page.getByRole('button',{name:'Cancel'}).click();
   await expect(page.locator('#install-dialog')).not.toBeVisible();
 }
});

test('connect validates, preserves input after error, and appears in My Apps', async ({page}) => {
 await page.getByRole('button',{name:'Connect an app'}).first().click();
 await page.getByLabel('App name',{exact:true}).fill('Home photos'); await page.getByLabel('Instance address').fill('file:///etc/passwd');
 await page.getByRole('button',{name:/Add to My Apps/}).click(); await expect(page.getByRole('alert')).toContainText('http:// or https://');
 await expect(page.getByLabel('App name',{exact:true})).toHaveValue('Home photos');
 await page.getByLabel('Instance address').fill('http://192.168.1.5:2283'); await page.getByRole('button',{name:/Add to My Apps/}).click();
 await expect(page.getByRole('heading',{name:'Right where you left them.'})).toBeVisible(); await expect(page.getByRole('heading',{name:'Home photos'})).toBeVisible();
 expect(await page.evaluate(() => window.__calls.find(c=>c.command==='add_app'))).toEqual({command:'add_app',args:{name:'Home photos',url:'http://192.168.1.5:2283'}});
});

test('My Apps handles open errors and confirms non-destructive removal', async ({page}) => {
 await page.getByRole('button',{name:/My Apps/}).click(); await page.getByRole('button',{name:/Open/}).click();
 // Readiness may finish after Open; assert the action itself, not IPC ordering.
 expect(await page.evaluate(() => window.__calls.filter(call => call.command === 'open_app')))
   .toEqual([{command:'open_app', args:{id:'studio-notes'}}]);
 await page.getByRole('button',{name:'Remove Studio notes'}).click();
 await expect(page.getByText('The server and its data stay untouched.')).toBeVisible(); await page.getByRole('button',{name:'Remove connection'}).click();
 await expect(page.getByRole('heading',{name:'Your apps belong here.'})).toBeVisible();
});

test('managed apps expose lifecycle, logs, and data-preserving uninstall', async ({page}) => {
 await installAdapter(page, {apps:[{id:'memos',display_name:'Memos',launch_url:'http://localhost:5230',icon_path:null,runtime:{kind:'compose'},status:'running',catalog_id:'Memos',created_at_unix:1,updated_at_unix:1}]});
 await page.goto('/'); await page.getByRole('button',{name:/My Apps/}).click();
 await page.getByRole('button',{name:'Logs'}).click(); await expect(page.getByText(/server started/)).toBeVisible(); await page.getByRole('button',{name:'Done'}).click();
 await page.getByRole('button',{name:'Stop'}).click(); await expect(page.getByRole('button',{name:'Start'})).toBeVisible();
 await page.getByRole('button',{name:'Uninstall Memos'}).click(); await expect(page.getByText('data is preserved by default')).toBeVisible();
 await page.getByRole('button',{name:'Uninstall, keep data'}).click();
 expect(await page.evaluate(() => window.__calls.find(c=>c.command==='uninstall_app'))).toEqual({command:'uninstall_app',args:{id:'memos',deleteData:false}});
});

test('keyboard shortcut and accessibility', async ({page}) => {
 await page.keyboard.press('Control+k'); await expect(page.getByRole('searchbox')).toBeFocused();
 await page.keyboard.press('Escape');
 const results = await new AxeBuilder({page}).analyze(); expect(results.violations).toEqual([]);
});

test('responsive visual surfaces', async ({page}) => {
 await expect(page).toHaveScreenshot('discover-1280x800.png', {animations:'disabled'});
 await page.setViewportSize({width:1180,height:640}); await expect(page).toHaveScreenshot('discover-1180x640.png', {animations:'disabled'});
 expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('dark-only surfaces keep contrast in light and dark system settings', async ({page}) => {
 for (const colorScheme of ['light', 'dark']) {
   await page.emulateMedia({colorScheme});
   expect(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe('dark');
   expect(await page.evaluate(() => getComputedStyle(document.documentElement).backgroundColor)).toBe('rgb(12, 10, 9)');
 }
 await page.getByRole('button',{name:'Connect an app',exact:true}).first().click();
 await expect(page.getByLabel('App name',{exact:true})).toBeFocused();
 expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
 await expect(page).toHaveScreenshot('connect-dialog.png', {animations:'disabled'});
 await page.keyboard.press('Escape');
 await expect(page.locator('#connect-dialog')).not.toBeVisible();
 await expect(page.locator('#connect-top')).toBeFocused();
});

test('keyboard and reduced-motion interactions stay immediate and restore focus', async ({page}) => {
 await page.locator('#connect-top').focus();
 await page.keyboard.press('Enter');
 await expect(page.getByLabel('App name',{exact:true})).toBeFocused();
 expect(await page.locator('#connect-dialog').evaluate(dialog => dialog.getAnimations().length)).toBe(0);
 await page.keyboard.press('Escape');
 await expect(page.locator('#connect-top')).toBeFocused();
 await page.emulateMedia({reducedMotion:'reduce'});
 await page.getByRole('button',{name:'Review n8n installation',exact:true}).click();
 await expect(page.getByRole('button',{name:'Install n8n',exact:true})).toBeEnabled();
 expect(await page.locator('#install-dialog').evaluate(dialog => dialog.getAnimations().length)).toBe(0);
 expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
 await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await expect(page.getByRole('button',{name:'Review n8n installation',exact:true})).toBeFocused();
});

test('Escape interrupts dialog entry without leaving the workspace inert', async ({page}) => {
 await page.locator('#connect-top').click();
 await page.keyboard.press('Escape');
 await expect(page.locator('#connect-dialog')).not.toBeVisible();
 await page.keyboard.press('Control+k');
 await expect(page.getByRole('searchbox')).toBeFocused();
 expect(await page.locator('#connect-dialog').evaluate(dialog => dialog.getAnimations().length)).toBe(0);
});

test('featured recipes require ready prerequisites and preserve errors for retry', async ({page}) => {
 await page.evaluate(() => {
   const invoke = window.__TAURI__.core.invoke;
   window.__TAURI__.core.invoke = (command,args) => command === 'doctor'
     ? Promise.resolve({ready:false,checks:[{label:'Docker engine',ok:false,detail:'Docker is not running'}]})
     : invoke(command,args);
 });
 await page.getByRole('button',{name:'Review Memos installation',exact:true}).click();
 await expect(page.getByText('Docker is not running', {exact:true})).toBeVisible();
 await expect(page.getByRole('button',{name:'Install Memos',exact:true})).toBeDisabled();
 expect(await page.evaluate(() => window.__calls.some(call=>call.command==='install_app'))).toBe(false);
 await page.keyboard.press('Escape');
 await page.evaluate(() => {
   const invoke = window.__TAURI__.core.invoke;
   window.__TAURI__.core.invoke = (command,args) => {
     if (command === 'doctor') return Promise.resolve({ready:true,checks:[{label:'Docker engine',ok:true,detail:'Ready'}]});
     if (command === 'install_app') return Promise.reject(new Error('Port 5230 is already in use.'));
     return invoke(command,args);
   };
 });
 await page.getByRole('button',{name:'Review Memos installation',exact:true}).click();
 await page.getByRole('button',{name:'Install Memos',exact:true}).click();
 await expect(page.getByText('Port 5230 is already in use.')).toBeVisible();
 await expect(page.getByRole('button',{name:'Install Memos',exact:true})).toBeEnabled();
 await expect(page.getByRole('button',{name:'Cancel',exact:true})).toBeEnabled();
});

test('reviewed apps can also connect an existing instance', async ({page}) => {
 await page.getByRole('button',{name:'View Memos details',exact:true}).click();
 await page.getByRole('button',{name:'Already running it? Connect an instance',exact:true}).click();
 await expect(page.getByLabel('App name',{exact:true})).toHaveValue('Memos');
 await expect(page.getByLabel('Instance address')).toHaveValue('');
 await expect(page.getByLabel('Instance address')).toBeFocused();
});

test('review and personal workspace visual surfaces', async ({page}) => {
 await page.getByRole('button',{name:'Review n8n installation',exact:true}).click();
 await expect(page.getByRole('button',{name:'Install n8n',exact:true})).toBeEnabled();
 await expect(page).toHaveScreenshot('install-review.png', {animations:'disabled'});
 await page.keyboard.press('Escape');
 await page.getByRole('button',{name:'My Apps',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Studio notes',exact:true})).toBeVisible();
 await expect(page).toHaveScreenshot('my-apps.png', {animations:'disabled'});
});


test('structured command failure stays readable and preserves connection input', async ({page}) => {
 await page.evaluate(() => {
   const original = window.__TAURI__.core.invoke;
   window.__TAURI__.core.invoke = async (command, args) => {
     if (command === 'add_app') throw {code: 'storage_io', message: 'Could not save your app. Check folder access.'};
     return original(command, args);
   };
 });
 await page.getByRole('button', {name:'Connect an app'}).first().click();
 await page.getByLabel('App name',{exact:true}).fill('Home photos');
 await page.getByLabel('Instance address').fill('http://192.168.1.5:2283');
 await page.getByRole('button', {name:/Add to My Apps/}).click();
 await expect(page.getByRole('alert')).toContainText('Could not save your app. Check folder access.');
 await expect(page.getByLabel('App name',{exact:true})).toHaveValue('Home photos');
 await expect(page.getByRole('button', {name:/Add to My Apps/})).toBeEnabled();
});
