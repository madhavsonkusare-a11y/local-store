import {test,expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';

for (const size of [{width:1280,height:800},{width:1180,height:640}]) {
  test(`approved shell and direct card actions remain usable at ${size.width}x${size.height}`, async ({page}) => {
    await page.setViewportSize(size);
    await installAdapter(page,{catalog:[{name:'Existing notes',description:'A place for writing.',category:'Notes',license:'MIT',architectures:['amd64','arm64'],capability:'connect'}],apps:[{id:'one',display_name:'One',launch_url:'http://localhost:8080',runtime:{kind:'external'},status:'connected'},{id:'two',display_name:'Two',launch_url:'http://localhost:8081',runtime:{kind:'external'},status:'connected'}]});
    await page.goto('/'); await page.evaluate(()=>document.fonts.ready);
    const card=page.locator('.app-card');
    await expect(card.locator('.card-facts')).toHaveText('MITamd64 · arm64');
    await expect(page.locator('#nav-discover')).toHaveAttribute('aria-current','page');
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
    await page.screenshot({animations:'disabled',path:`.cache/v2-shell-discover-${size.width}.png`});
    // Exercise the footer action itself: the previous full-card details overlay
    // could intercept it while keyboard activation still worked.
    await card.getByRole('button',{name:'Connect',exact:true}).click();
    await expect(page.locator('#connect-dialog')).toBeVisible();
    await expect(page.locator('#connect-name')).toHaveValue('Existing notes');
    expect(await page.evaluate(()=>window.__calls.some(call=>['install_app','save_external_app'].includes(call.command)))).toBe(false);
    await page.getByRole('button',{name:'Cancel',exact:true}).click();
    await page.getByRole('button',{name:'My Apps',exact:true}).click();
    const row=page.locator('.installed-app').filter({has:page.getByRole('button',{name:'Select Two',exact:true})});
    const rowHeight=await row.evaluate(node=>node.getBoundingClientRect().height);
    expect(rowHeight).toBeGreaterThanOrEqual(68); expect(rowHeight).toBeLessThanOrEqual(72);
    await page.getByRole('button',{name:'Select Two',exact:true}).click();
    await expect(page.locator('#my-apps-detail-title')).toHaveText('Two');
    await page.screenshot({animations:'disabled',path:`.cache/v2-shell-my-apps-${size.width}.png`});
    expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
    await page.getByRole('button',{name:'Overview',exact:true}).click();
    await expect(page.locator('.overview-engine-summary')).toContainText('Managed engine');
    await page.screenshot({animations:'disabled',path:`.cache/v2-shell-overview-${size.width}.png`});
    await page.getByRole('button',{name:'View settings',exact:true}).click();
    await expect(page).toHaveURL(/#settings$/);
    await expect(page.locator('#settings-title')).toBeFocused();
    await page.screenshot({animations:'disabled',path:`.cache/v2-shell-settings-${size.width}.png`});
  });
}
