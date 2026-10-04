import {test,expect} from '@playwright/test';
import {installAdapter} from './fixtures.js';
for(const route of ['discover','my-apps','overview','activity','settings']) test(`production ${route} visible reading labels meet frozen type floor`,async({page})=>{
  await installAdapter(page);await page.goto(`/#${route}`);await expect(page.locator('[aria-busy="true"]:visible')).toHaveCount(0);
  const small=await page.evaluate(()=>[...document.querySelectorAll('body *')].filter(el=>!el.closest('[hidden],[aria-hidden="true"]')&&[...el.childNodes].some(node=>node.nodeType===3&&node.textContent.trim())&&el.getBoundingClientRect().width&&el.getBoundingClientRect().height).map(el=>{const s=getComputedStyle(el);return {where:el.tagName.toLowerCase()+(el.id?'#'+el.id:'.'+[...el.classList].join('.')),text:el.textContent.trim().slice(0,50),size:parseFloat(s.fontSize),mono:s.fontFamily.includes('Mono'),tracking:parseFloat(s.letterSpacing)||0};}).filter(x=>x.size<13&&!(x.mono&&x.size>=11&&x.tracking>0)));
  expect(small).toEqual([]);
});
