import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { readFileSync } from 'node:fs';
import { installAdapter } from './fixtures.js';

const snapshot = JSON.parse(readFileSync(new URL('../../src/generated/catalog.json', import.meta.url), 'utf8'));
const productionCatalog = snapshot.entries.map(app => ({...app, license: app.licenses.join(', '), capability: ['memos','n8n','uptime-kuma'].includes(app.id) ? 'preview_install' : app.web_ui ? 'connect' : 'discover', recipe_id: ['memos','n8n','uptime-kuma'].includes(app.id) ? app.id : null}));
test.beforeEach(async ({page}) => { await installAdapter(page, {catalog:productionCatalog}); await page.goto('/'); });

test('production catalog pages, combines filters and keeps provenance', async ({page}) => {
  await expect(page.locator('#catalog-note')).toContainText(`${productionCatalog.length.toLocaleString('en-US')} projects`);
  await expect(page.locator('.app-card')).toHaveCount(24);
  const first = await page.locator('.app-card h3').allTextContents();
  await page.getByRole('button',{name:'Load next 24',exact:true}).click();
  await expect(page.locator('#page-label')).toContainText('Showing 48');
  expect((await page.locator('.app-card h3').allTextContents()).slice(0,24)).toEqual(first);
  await page.getByRole('button',{name:'Install previews',exact:true}).click();
  await expect(page.locator('.app-card')).toHaveCount(3);
  await expect(page.locator('#next')).toBeHidden();
  await page.getByRole('searchbox',{name:'Search apps',exact:true}).fill('memos');
  await expect(page.locator('.app-card')).toHaveCount(1);
  await page.getByRole('button',{name:'View Memos details'}).click();
  await page.locator('.provenance summary').click();
  await expect(page.getByRole('button',{name:'awesome-selfhosted',exact:false})).toBeVisible();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
});

test('category picker searches, cancels drafts, traps keyboard focus and combines filters', async ({page}) => {
  await page.getByRole('button',{name:'Filters',exact:true}).click();
  await page.getByLabel('Find a category').fill('Analytics');
  await page.getByRole('button',{name:/^Analytics/}).click();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button',{name:'Filters',exact:true})).toBeFocused();
  await expect(page.locator('#active-filters')).toBeEmpty();
  await page.getByRole('button',{name:'Filters',exact:true}).click();
  await page.getByRole('button',{name:'Show results'}).focus();
  await page.keyboard.press('Tab');
  // Chromium may visit browser chrome between the last and first modal controls.
  if (await page.evaluate(() => document.activeElement === document.body)) await page.keyboard.press('Tab');
  await expect(page.getByRole('button',{name:'Close filters'})).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  if (await page.evaluate(() => document.activeElement === document.body)) await page.keyboard.press('Shift+Tab');
  await expect(page.getByRole('button',{name:'Show results'})).toBeFocused();
  await page.getByLabel('Find a category').fill('Analytics');
  await page.getByRole('button',{name:/^Analytics/}).click();
  await page.getByLabel('Software license').selectOption('MIT');
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await expect(page).toHaveScreenshot('catalog-filters.png', {animations:'disabled'});
  await page.getByRole('button',{name:'Show results'}).click();
  await expect(page.locator('.card-category').first()).toHaveText('Analytics');
  await expect(page.locator('#active-filters')).toContainText('MIT');
  await page.getByRole('button',{name:'Reset filters',exact:true}).click();
  await expect(page.locator('#active-filters')).toBeEmpty();
});

test('catalog artwork is local and image failure leaves a usable card', async ({page}) => {
  const remote = [];
  page.on('request', request => { if (request.resourceType() === 'image' && !request.url().startsWith('http://127.0.0.1')) remote.push(request.url()); });
  await page.getByRole('searchbox',{name:'Search apps',exact:true}).fill('Jellyfin');
  const matches = productionCatalog.filter(app => `${app.name} ${app.description} ${app.aliases.join(' ')}`.toLowerCase().includes('jellyfin'));
  await expect(page.locator('.app-card')).toHaveCount(matches.length);
  await expect(page.getByRole('button',{name:'View Jellyfin details',exact:true})).toBeVisible();
  const card = page.locator('.app-card').filter({has:page.getByRole('button',{name:'View Jellyfin details',exact:true})});
  const image = card.locator('.app-avatar img');
  await expect(image).toHaveAttribute('src', /^assets\//);
  await image.evaluate(element => element.dispatchEvent(new Event('error', {bubbles:false})));
  await expect(card.locator('.app-avatar img')).toHaveCount(0);
  await expect(page.getByRole('button',{name:'View Jellyfin details',exact:true})).toBeEnabled();
  expect(remote).toEqual([]);
});

test('settings exposes diagnostics and handles failure', async ({page}) => {
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('button',{name:'Run check'}).click();
  await expect(page.getByText('Docker engine', {exact:true})).toBeVisible();
  expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
  await expect(page).toHaveScreenshot('settings.png', {animations:'disabled'});
  await page.evaluate(() => { window.__TAURI__.core.invoke = () => Promise.reject(new Error('Docker diagnostics unavailable')); });
  await page.getByRole('button',{name:'Run again'}).click();
  await expect(page.locator('#doctor-error')).toHaveText('Docker diagnostics unavailable');
  await expect(page.getByRole('button',{name:'Run again'})).toBeEnabled();
});

test('managed-engine repair appears only for an owned unresponsive engine', async ({page}) => {
  await page.evaluate(() => {
    const original = window.__TAURI__.core.invoke;
    let responsive = false;
    window.__TAURI__.core.invoke = async (command, args) => {
      if (command === 'managed_engine_status') return {
        bootstrap:{status:'recovery', phase:'verified', disposition:'ready'},
        prerequisites:{state:'ready'}, daemon:responsive ? 'responsive' : 'unresponsive',
        disk_available_bytes:1000000,
      };
      if (command === 'repair_managed_engine') { responsive = true; return true; }
      return original(command, args);
    };
  });
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await expect(page.locator('#engine-output')).toContainText('Needs attention');
  await page.getByRole('button',{name:'Repair managed engine'}).click();
  await expect(page.locator('#engine-output')).toContainText('Ready');
  await expect(page.getByRole('button',{name:'Repair managed engine'})).toBeHidden();
});

test('new SVG and PNG identities render in cards and details offline', async ({page}) => {
  await page.route('https://**', route => route.abort());
  for (const name of ['Ampache', 'Appsmith', 'Dashy']) {
    await page.getByRole('searchbox',{name:'Search apps',exact:true}).fill(name);
    const button = page.getByRole('button',{name:`View ${name} details`,exact:true});
    const card = page.locator('.app-card').filter({has:button});
    await expect(card.locator('.app-avatar img')).toHaveAttribute('src', /^assets\/catalog\/[a-z0-9-]+\.(svg|png)$/);
    await expect.poll(() => card.locator('.app-avatar img').evaluate(image => image.complete && image.naturalWidth > 0)).toBe(true);
    await button.click();
    await expect.poll(() => page.locator('.detail-heading img').evaluate(image => image.complete && image.naturalWidth > 0)).toBe(true);
    await page.getByRole('button',{name:'Back to collection',exact:true}).click();
    await expect(page.locator('#detail-dialog')).not.toBeVisible();
  }
});

test('all bundled catalog artwork decodes without remote requests', async ({page}) => {
  test.setTimeout(60_000);
  await page.route('https://**', route => route.abort());
  const paths = [...new Set(productionCatalog.map(app => app.icon).filter(Boolean))];
  const failures = await page.evaluate(async paths => {
    const failures = [];
    // Small batches avoid overwhelming the preview server or image decoder.
    for (let offset = 0; offset < paths.length; offset += 24) {
      await Promise.all(paths.slice(offset, offset + 24).map(async path => {
        const image = new Image();
        image.src = path;
        try { await image.decode(); } catch { failures.push(path); }
      }));
    }
    return failures;
  }, paths);
  expect(failures).toEqual([]);
});

test('full catalog visual surfaces', async ({page}) => {
  await expect(page.locator('.app-card')).toHaveCount(24);
  await expect(page).toHaveScreenshot('catalog-desktop.png', {animations:'disabled'});
  await page.setViewportSize({width:1180,height:640});
  await expect(page).toHaveScreenshot('catalog-compact.png', {animations:'disabled'});
  // Below 1180px is outside the approved V2 window contract.
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
