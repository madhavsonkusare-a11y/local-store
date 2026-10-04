import {test, expect} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {installAdapter} from './fixtures.js';
import {compareRaster} from './raster-comparison.js';

// Software rasterization avoids device-dependent GPU edge dithering in exact
// PNG comparisons; this remains a browser receipt, not native WebView proof.
test.use({launchOptions:{args:['--disable-gpu','--disable-gpu-compositing','--disable-skia-runtime-opts','--force-color-profile=srgb']}});

// Production controllers with explicit IPC fixtures. This verifies browser
// composition only; it is not a native WebView or clean-machine receipt.
const sizes = [
  {width:1440,height:900,scale:1},
  {width:1280,height:800,scale:1},
  {width:1280,height:640,scale:1.5},
];
const routes = ['discover','overview','my-apps','activity','settings'];

for (const size of sizes) for (const route of routes) {
  test(`local V2 ${route} ${size.width}x${size.height} at ${size.scale}x is accessible with exact geometry and capture`, async ({browser},testInfo) => {
    const captures=[],geometry=[];
    for (let repetition=0;repetition<2;repetition++) {
      const context=await browser.newContext({viewport:{width:size.width,height:size.height},deviceScaleFactor:size.scale,reducedMotion:'reduce'});
      try {
        const page=await context.newPage();
        const external=[],assetErrors=[];
        page.on('requestfailed',request=>assetErrors.push(request.url()));
        page.on('response',response=>{if(response.status()>=400)assetErrors.push(response.url());});
        await page.route('**/*',async request=>{
          const url=new URL(request.request().url());
          if(url.protocol==='http:' && url.hostname==='127.0.0.1') return request.continue();
          if(['http:','https:'].includes(url.protocol)) {external.push(url.origin);return request.abort();}
          return request.continue();
        });
        await installAdapter(page);
        await page.goto(`/#${route}`);
        await page.evaluate(()=>document.fonts.ready);
        await expect(page.locator('#rail-engine-state')).not.toHaveText('Checking…');
        await expect(page.locator('[aria-busy="true"]:visible')).toHaveCount(0);
        await expect(page.locator('#settings-dialog')).toBeVisible({visible:route==='settings'});
        // Screenshot disables finite animations and caret. Two independent
        // contexts retain their entire images and must match byte-for-byte.
        await page.evaluate(async()=>{await Promise.all([...document.images].filter(image=>{const r=image.getBoundingClientRect();return r.width>0&&r.height>0&&r.top<innerHeight;}).map(image=>image.decode().catch(()=>{})));const finite=document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime));await Promise.race([Promise.all(finite.map(animation=>animation.finished.catch(()=>{}))),new Promise(resolve=>setTimeout(resolve,1000))]);await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));});
        captures.push(await page.screenshot({animations:'disabled',caret:'hide',path:`.cache/v2-local-${route}-${size.width}-${size.height}-${repetition}.png`}));
        geometry.push(await page.evaluate(()=>[...document.querySelectorAll('button,input,select,h1,h2,.app-card,.environment-card,.status-dot')].filter(el=>{const r=el.getBoundingClientRect();return r.width>0&&r.height>0;}).map(el=>{const r=el.getBoundingClientRect(),s=getComputedStyle(el);return {text:el.textContent.trim(),x:r.x,y:r.y,width:r.width,height:r.height,color:s.color,background:s.backgroundColor,border:s.borderColor,font:s.font,shadow:s.boxShadow};})));
        expect(external).toEqual([]);
        expect(assetErrors).toEqual([]);
        if(repetition===0) {
          expect((await new AxeBuilder({page}).analyze()).violations).toEqual([]);
          const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>window.innerWidth);
          expect(overflow).toBe(false);
          await page.locator('#nav-discover').focus();
          await page.keyboard.press('Enter');
          await expect(page.locator('#nav-discover')).toHaveAttribute('aria-current','page');
          await page.locator('#settings').focus();
          await page.keyboard.press('Enter');
          await expect(page.locator('#settings-title')).toBeFocused();
        }
      } finally {await context.close();}
    }
    expect(geometry[0]).toEqual(geometry[1]);
    const raster=compareRaster(captures[0],captures[1]);
    await testInfo.attach('raster-comparison',{body:JSON.stringify(raster),contentType:'application/json'});
    // The frozen handoff requires byte identity. Report any variance without
    // masks, crops, relaxed bounds or an inferred waiver.
    expect(raster.byteIdentical).toBe(true);
  });
}
