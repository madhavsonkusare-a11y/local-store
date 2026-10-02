// Only synthetic, isolated qualification users. This is proof setup, not a
// production browser provider or an automatic agent configuration flow.
import {chromium} from '@playwright/test';
import {readFile,writeFile} from 'node:fs/promises';

const [address,statePath,tokenPath] = process.argv.slice(2);
if (!statePath || !tokenPath) throw new Error('LOOPBACK_URL SYNTHETIC_STATE PRIVATE_TOKEN_FILE required');
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost','127.0.0.1','[::1]'].includes(base.hostname) || !base.port || base.username || base.password || base.pathname !== '/' || base.search || base.hash) throw new Error('unadorned loopback fixture address required');
const state = JSON.parse(await readFile(statePath,'utf8'));
if (state.schema !== 1 || !/^local-store-[a-f0-9]{16}@example\.invalid$/.test(state.email) || typeof state.password !== 'string' || !/^[a-zA-Z0-9_-]+$/.test(state.workflowId)) throw new Error('synthetic n8n qualification state required');
const browser = await chromium.launch({headless:true,channel:'msedge'});
try {
  const page = await browser.newPage(); page.setDefaultTimeout(30000);
  let sessionHeaders;
  page.on('request', request => {
    if (new URL(request.url()).pathname === '/rest/projects/my-projects') {
      const headers=request.headers(); if (headers['browser-id'] && headers['push-ref']) sessionHeaders={'browser-id':headers['browser-id'],'push-ref':headers['push-ref']};
    }
  });
  await page.goto(base.href,{waitUntil:'domcontentloaded'});
  await page.locator('#emailOrLdapLoginId').fill(state.email); await page.locator('#password').fill(state.password); await page.locator('[data-test-id="form-submit-button"]').click();
  await page.waitForFunction(()=>!document.querySelector('#emailOrLdapLoginId'));
  const deadline=Date.now()+30000; while (!sessionHeaders && Date.now()<deadline) await page.waitForTimeout(250);
  if (!sessionHeaders) throw new Error('synthetic n8n session unavailable');
  async function api(path,method='GET',body) {
    const result=await page.evaluate(async({path,method,body,sessionHeaders})=>{
      const response=await fetch(path,{method,credentials:'include',redirect:'manual',headers:{...sessionHeaders,...(body ? {'content-type':'application/json'} : {})},...(body ? {body:JSON.stringify(body)} : {})});
      const payload=await response.json(); return {status:response.status,payload};
    },{path,method,body,sessionHeaders});
    if (result.status!==200 || !result.payload?.data) throw new Error('synthetic MCP setup request refused');
    return result.payload.data;
  }
  const settings=await api('/rest/mcp/settings','PATCH',{mcpAccessEnabled:true,autoExposeNewWorkflows:false});
  if (settings.mcpAccessEnabled!==true || settings.autoExposeNewWorkflows!==false) throw new Error('synthetic MCP enablement not confirmed');
  // Native workflow-detail reads require the owner's exact workflow to be
  // exposed. This fixture changes only its synthetic workflow, never a user's
  // instance or the global auto-expose policy.
  const workflow=await api(`/rest/workflows/${state.workflowId}`);
  const exposed=await api(`/rest/workflows/${state.workflowId}`,'PATCH',{settings:{...workflow.settings,availableInMCP:true},versionId:workflow.versionId});
  if (exposed.settings?.availableInMCP!==true) throw new Error('synthetic workflow MCP exposure was not confirmed');
  const key=await api('/rest/mcp/api-key');
  if (typeof key.apiKey!=='string' || key.apiKey.length<16 || key.apiKey.includes('*')) throw new Error('synthetic MCP token was not issued once');
  await writeFile(tokenPath,JSON.stringify({token:key.apiKey,workflowId:state.workflowId,workflowName:state.name}),{flag:'wx',mode:0o600});
  console.log('Synthetic native MCP setup passed');
} finally {await browser.close();}
