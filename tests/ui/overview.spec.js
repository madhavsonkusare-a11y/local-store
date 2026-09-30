import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

const apps = [
  {id:'notes', display_name:'My notes', launch_url:'http://localhost:5230', status:'connected', runtime:{kind:'external'}},
  {id:'tasks', display_name:'Tasks', launch_url:'http://localhost:8080', status:'stopped', runtime:{kind:'compose'}},
];

test('Overview derives counts and attention from saved apps', async ({page}) => {
  await installAdapter(page, {apps});
  await page.goto('/');
  await page.getByRole('button', {name:'Overview'}).click();
  await expect(page.getByRole('heading', {name:'A place for what runs here.'})).toBeFocused();
  await expect(page.locator('.overview-metrics strong')).toHaveText(['2', '1', '1', '1']);
  await expect(page.locator('.overview-app-row')).toHaveCount(1);
  await expect(page.locator('.overview-app-row')).toContainText('Tasks');
  await expect(page.locator('#overview-health-title')).toHaveText('Managed engine not configured');
  const results = await new AxeBuilder({page}).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
  expect(results.violations).toEqual([]);
  await page.getByRole('button', {name:'Review My Apps'}).click();
  await expect(page.locator('.installed-app')).toHaveCount(2);
});

test('Overview keeps a usable recovery route on app-list failure and is axe clean', async ({page}) => {
  await installAdapter(page, {failure:'list_apps'});
  await page.goto('/');
  await page.getByRole('button', {name:'Overview'}).click();
  await expect(page.getByRole('heading', {name:'Couldn’t load your apps.'})).toBeVisible();
  await expect(page.getByRole('button', {name:'Try again'})).toBeVisible();
  const results = await new AxeBuilder({page}).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
  expect(results.violations).toEqual([]);
});
