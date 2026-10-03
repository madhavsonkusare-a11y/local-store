import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

const repo = 'https://github.com/usememos/memos';
const inspection = resolution => ({requested_repository:repo, canonical_repository:repo, repository_id:42, default_branch:'main', commit_sha:'a'.repeat(40), commit_url:`${repo}/commit/${'a'.repeat(40)}`, archived:false, resolution});
async function setup(page, response, failure = false) {
  await installAdapter(page);
  await page.addInitScript(({response, failure}) => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = async (command, args) => {
      if (command !== 'resolve_github_source') return original(command, args);
      window.__calls.push({command,args});
      if (failure) throw new Error('GitHub rate-limited repository inspection; try later.');
      return response;
    };
  }, {response,failure});
  await page.goto('/'); await page.locator('#github-open').click();
}
async function check(page, value = repo) {
  await page.getByLabel('GitHub repository',{exact:true}).fill(value);
  await page.locator('#github-check').click();
}
test('exact approved GitHub match opens the existing install review without installing', async({page}) => {
  await setup(page, inspection({kind:'approved_match',offering_id:'memos',display_name:'Memos'}));
  await check(page);
  await expect(page.locator('#github-result')).toContainText('inspected commit aaaaaaaaaaaa');
  await expect(page.locator('#github-result')).toContainText('metadata, not installation approval');
  expect(await page.evaluate(() => window.__calls.some(c => c.command === 'install_app'))).toBe(false);
  const violations = (await new AxeBuilder({page}).include('#github-dialog').analyze()).violations;
  expect(violations).toEqual([]);
  if (process.env.LOCAL_STORE_CAPTURE_UI === '1') await page.screenshot({path:'.cache/ui/github-approved.png'});
  await page.locator('#github-dialog').getByRole('button',{name:'Review Memos installation',exact:true}).click();
  await expect(page.locator('#install-dialog')).toBeVisible();
  await expect(page.locator('#install-title')).toHaveText('Review Memos installation');
  expect(await page.evaluate(() => window.__calls.filter(c => c.command === 'recipe_details').map(c => c.args.id))).toEqual(['memos']);
  expect(await page.evaluate(() => window.__calls.some(c => c.command === 'install_app'))).toBe(false);
});
test('candidate provenance and blockers are shown without offering install', async({page}) => {
  const candidate = {source:'runtipi',id:'candidate',review_stage:'needs_qualification',service_count:2,required_inputs:1,revision:'b'.repeat(40),path:'apps/candidate/docker-compose.json',archive_sha256:'c'.repeat(64),definition_url:`https://github.com/runtipi/runtipi-appstore/blob/${'b'.repeat(40)}/apps/candidate/docker-compose.json`,image_references:['candidate/app:1.0'],remaining_checks:['Exact content survives restart','Owner promotion']};
  await setup(page, inspection({kind:'review_candidates',candidates:[candidate]})); await check(page);
  await expect(page.locator('#github-result')).toContainText('They are not installable yet');
  await expect(page.locator('#github-result')).toContainText('2 services · 1 required setup answer');
  await expect(page.locator('#github-result')).toContainText('Exact content survives restart');
  await page.getByText('Pinned definition and provenance',{exact:true}).click();
  await expect(page.locator('#github-result')).toContainText('c'.repeat(64));
  if (process.env.LOCAL_STORE_CAPTURE_UI === '1') await page.screenshot({path:'.cache/ui/github-candidate.png'});
  await page.getByRole('button',{name:'Open pinned definition',exact:true}).click();
  expect(await page.evaluate(() => window.__calls.filter(c => c.command === 'open_project').map(c => c.args.url))).toEqual([candidate.definition_url]);
  expect(await page.locator('#github-result button.primary').count()).toBe(0);
  expect(await page.evaluate(() => window.__calls.some(c => ['recipe_details','install_app'].includes(c.command)))).toBe(false);
});
test('unsafe input is refused before network lookup and GitHub failure offers retry', async({page}) => {
  await setup(page,null,true);
  await check(page,'https://user:private@github.com/usememos/memos');
  await expect(page.locator('#github-error')).toContainText('without credentials');
  expect(await page.evaluate(() => window.__calls.some(c => c.command === 'resolve_github_source'))).toBe(false);
  await check(page); await expect(page.locator('#github-error')).toContainText('rate-limited');
  await expect(page.locator('#github-check')).toBeEnabled();
  await expect(page.locator('#github-url')).toBeEnabled();
});
test('unsupported repository explains required review and exposes no installation action', async({page}) => {
  await setup(page,inspection({kind:'needs_review',repository:repo}));
  await check(page);
  await expect(page.locator('#github-result')).toContainText('No reviewed installation matches');
  await expect(page.locator('#github-result')).toContainText('source review and Windows app verification');
  expect(await page.locator('#github-result button.primary').count()).toBe(0);
  expect(await page.evaluate(() => window.__calls.some(c => ['recipe_details','install_app'].includes(c.command)))).toBe(false);
});
test('unknown review status fails closed and metadata is rendered as text', async({page}) => {
  await setup(page,inspection({kind:'<img src=x onerror=alert(1)>',offering_id:'memos'})); await check(page);
  await expect(page.locator('#github-error')).toContainText('unknown review status');
  await expect(page.locator('#github-result')).toHaveText('');
  expect(await page.evaluate(() => window.__calls.some(c => ['recipe_details','install_app'].includes(c.command)))).toBe(false);
});
test('closed inspection cannot repopulate a reopened dialog or revive install approval', async({page}) => {
  await setup(page,inspection({kind:'needs_review'}));
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (command,args) => command === 'resolve_github_source'
      ? new Promise(resolve => {window.__releaseGithub = resolve;}) : original(command,args);
  });
  await check(page); await expect(page.locator('#github-check')).toBeDisabled();
  await page.keyboard.press('Escape'); await expect(page.locator('#github-dialog')).toBeHidden();
  await page.locator('#github-open').click();
  await page.evaluate(value => window.__releaseGithub(value),inspection({kind:'approved_match',offering_id:'memos',display_name:'Memos'}));
  await expect(page.locator('#github-result')).toHaveText('');
  await expect(page.locator('#github-check')).toBeEnabled();
  await expect(page.locator('#github-url')).toHaveValue('');
});
