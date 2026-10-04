import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

for (const scenario of [
  {name:'ready',report:{prerequisites:{state:'ready'},bootstrap:{disposition:'ready'},daemon:'responsive'},label:'Engine ready',tone:'success'},
  {name:'unresponsive',report:{prerequisites:{state:'ready'},bootstrap:{disposition:'ready'},daemon:'unresponsive'},label:'Needs attention',tone:'warning'},
  {name:'ownership mismatch',report:{prerequisites:{state:'ready'},bootstrap:{status:'recovery',disposition:'manual_review'},daemon:'not_checked'},label:'Manual review',tone:'warning'},
  {name:'missing prerequisite',report:{prerequisites:{state:'missing'},bootstrap:{status:'not_configured'},daemon:'not_checked'},label:'Windows setup needed',tone:'warning'},
  {name:'read failure',failure:true,label:'Check failed',tone:'warning'},
]) test(`rail reports factual engine ${scenario.name} without running setup`,async({page})=>{
  await installAdapter(page);
  await page.addInitScript(s=>{const invoke=window.__TAURI__.core.invoke;window.__TAURI__.core.invoke=async(c,a)=>{if(c==='managed_engine_status'){window.__calls.push({command:c,args:a});if(s.failure)throw new Error('Unavailable');return s.report;}return invoke(c,a);};},scenario);
  await page.goto('/');
  await expect(page.locator('#rail-engine-state')).toHaveText(scenario.label);
  await expect(page.locator('#rail-engine')).toHaveAttribute('data-tone',scenario.tone);
  await page.locator('#rail-engine').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#settings-title')).toBeFocused();
  expect(await page.evaluate(()=>window.__calls.filter(c=>/setup_managed_engine|repair_managed_engine|install_app/.test(c.command)))).toEqual([]);
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
});

test('real pending reads show approved loading shapes, visible focus and static reduced motion',async({page})=>{
  await page.emulateMedia({reducedMotion:'reduce'});
  await installAdapter(page);
  await page.addInitScript(()=>{const invoke=window.__TAURI__.core.invoke;window.__hold=false;window.__TAURI__.core.invoke=async(c,a)=>{if(window.__hold&&['list_apps','managed_engine_status','search_catalog','recipe_details'].includes(c))return new Promise(resolve=>{(window.__releaseReads??=[]).push(async()=>resolve(await invoke(c,a)));});return invoke(c,a);};});
  await page.goto('/');
  await page.evaluate(()=>{window.__hold=true;});
  await page.locator('#nav-overview').click();
  await expect(page.locator('#overview-content .skeleton.hero')).toBeVisible();
  await expect(page.locator('#overview-content')).toHaveAttribute('aria-busy','true');
  await expect(page.locator('#overview-refresh')).toBeDisabled();
  expect(await page.locator('.skeleton').first().evaluate(el=>getComputedStyle(el,'::after').animationName)).toBe('none');
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await page.evaluate(()=>{window.__hold=false;window.__releaseReads.splice(0).forEach(release=>release());});
  await expect(page.locator('#overview-content')).toHaveAttribute('aria-busy','false');
  await page.evaluate(()=>{window.__hold=true;});
  await page.locator('#nav-apps').click();
  await expect(page.locator('.app-row-skeleton')).toHaveCount(5);
  await page.keyboard.press('Tab');
  await page.locator('#rail-engine').focus();
  expect(await page.locator('#rail-engine').evaluate(el=>getComputedStyle(el).outlineStyle)).toBe('solid');
  await page.evaluate(()=>{window.__hold=false;window.__releaseReads.splice(0).forEach(release=>release());});
  await expect(page.locator('.installed-app')).toHaveCount(1);
});
