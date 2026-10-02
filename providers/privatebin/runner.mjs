// Reviewed fixed DOM actions only. This process runs non-root with no network,
// no host browser/profile/socket, read-only code, and a temporary private profile.
import {chromium} from '/provider/playwright-core/index.mjs';
let raw='';for await(const chunk of process.stdin){raw+=chunk;if(Buffer.byteLength(raw)>16*1024*1024)process.exit(2);}
const input=JSON.parse(raw);
// This exact pinned Noble image assigns pwuser UID 1001.
if(process.getuid()!==1001)process.exit(2);
if (!['read','create'].includes(input.operation) || typeof input.html!=='string' || !input.assets || typeof input.assets!=='object') process.exit(2);
const base=new URL(input.address);
if(base.protocol!=='http:'||base.hostname!=='127.0.0.1'||!base.port||base.pathname!=='/'||base.username||base.password||base.search||base.hash)process.exit(2);
const browser=await chromium.launch({headless:true,chromiumSandbox:true});
let intercepted,blocked=0;
try{
  const context=await browser.newContext({acceptDownloads:false,serviceWorkers:'block'});
  await context.routeWebSocket('**/*',route=>{blocked++;route.close();});
  await context.route('**/*',async route=>{
    const request=route.request();const url=new URL(request.url());
    if(url.origin!==base.origin||!['GET','POST'].includes(request.method())){blocked++;return route.abort();}
    if(request.isNavigationRequest()){
      if(url.pathname!=='/'||request.frame().parentFrame()){blocked++;return route.abort();}
      return route.fulfill({status:200,contentType:'text/html; charset=utf-8',body:input.html});
    }
    if(request.method()==='POST'){
      if(input.operation!=='create'||url.pathname!=='/'||url.search||intercepted){blocked++;return route.abort();}
      intercepted=JSON.parse(request.postData());
      return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({status:0,id:'0000000000000000',deletetoken:'unavailable'})});
    }
    if(url.pathname==='/'&&url.searchParams.get('pasteid')===input.paste_id&&input.operation==='read'){
      return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify(input.encrypted)});
    }
    const asset=input.assets[url.pathname+url.search];
    if(!asset){blocked++;return route.abort();}
    return route.fulfill({status:200,contentType:asset.type,body:asset.body});
  });
  const page=await context.newPage();page.setDefaultTimeout(30000);
  page.on('popup',popup=>popup.close());page.on('download',download=>download.cancel());
  if(input.operation==='read'){
    if(!/^[a-f0-9]{16}$/.test(input.paste_id)||typeof input.fragment!=='string'||!/^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(input.fragment))throw Error('reviewed paste identity required');
    await page.goto(`${base.href}?${input.paste_id}#${input.fragment}`);
    await page.locator('#prettyprint:visible').waitFor();
    const content=await page.locator('#prettyprint').textContent();if(Buffer.byteLength(content)>8192)throw Error('paste exceeds provider limit');
    process.stdout.write(JSON.stringify({content,blocked_requests:blocked}));
  }else{
    if(typeof input.content!=='string'||!input.content.trim()||Buffer.byteLength(input.content)>8192)throw Error('bounded text required');
    await page.goto(base.href);await page.locator('#message').fill(input.content);
    const expiry=page.locator('#pasteExpiration');
    if(await expiry.count())await expiry.selectOption('1day');
    else await page.locator('[data-expiration="1day"]').click({force:true});
    for(const id of ['burnafterreading','opendiscussion']){const choice=page.locator(`#${id}`);if(await choice.count())await choice.uncheck();}
    await page.locator('#sendbutton:visible').click();await page.waitForFunction(()=>Boolean(location.search&&location.hash));
    const fragment=new URL(page.url()).hash.slice(1);
    if(!intercepted||!/^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(fragment))throw Error('paste encryption did not complete');
    process.stdout.write(JSON.stringify({payload:intercepted,fragment,blocked_requests:blocked}));
  }
}catch{process.exitCode=2;}finally{await browser.close();}
