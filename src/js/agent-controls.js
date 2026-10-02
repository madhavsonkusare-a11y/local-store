import {invoke} from './api.js';

// Owner controls live only in the bundled launcher. Configuration is revealed
// once on an explicit click; it never enters storage, logs, toast or clipboard.
const settings = document.getElementById('settings-dialog');
const section = document.createElement('section');
section.className = 'agent-settings';
section.innerHTML = `<div class="settings-row"><div><h3>Agent connections</h3><p>Choose which installed apps an agent can manage.</p></div><button class="secondary" id="agent-refresh">Manage agents</button></div>
<div id="agent-panel" hidden><p class="field-hint">Connections start with no permissions. Reviewed recipe installs and uninstalling while keeping data need your approval for each request. Manage Memos content permissions separately below. App data deletion is not available.</p>
<form id="agent-enroll-form"><label class="field">Agent name<input id="agent-client-name" required pattern="[a-z0-9][a-z0-9-]*" maxlength="80" autocomplete="off" placeholder="e.g. hermes"></label>
<label class="delete-choice"><input id="agent-enroll-consent" type="checkbox" required><span>Create a local connection. I will review and paste its configuration into my agent.</span></label>
<button class="secondary" type="submit">Create connection</button></form>
<div id="agent-preview" hidden><p id="agent-preview-note"></p><label class="delete-choice"><input id="agent-export-consent" type="checkbox"><span>Show the private connection credential once. Keep this configuration private.</span></label>
<button class="secondary" id="agent-export">Show configuration</button><button class="text-button" id="agent-cancel">Cancel connection</button><textarea id="agent-configuration" class="log-view" aria-label="Private agent configuration" readonly hidden spellcheck="false"></textarea></div>
<div id="agent-clients"></div><form id="agent-grant-form"><h3>App permissions</h3><label class="field">Connection<select id="agent-grant-client" required></select></label><label class="field">Installed app<select id="agent-grant-app" required></select></label>
<label class="field">Allowed actions<select id="agent-grant-scope"><option value="status">View status</option><option value="lifecycle">View status, start and stop</option></select></label><label class="field">Expires after (hours)<input id="agent-grant-hours" type="number" min="1" max="720" value="1" required></label>
<label class="delete-choice"><input id="agent-grant-consent" type="checkbox" required><span>Allow these actions for this app until the permission expires.</span></label><button class="secondary" id="agent-grant-submit" type="submit">Save permission</button></form>
<div id="agent-grants"></div><h3>Requests needing review</h3><p class="field-hint">Approval lasts 10 minutes and can be used once. An install can download images and create an app; an uninstall keeps its data. Setup-assisted apps require installation in the launcher.</p><div id="agent-requests"></div><h3>Recent access</h3><div id="agent-audit"></div><p id="agent-boundary" class="field-hint"></p></div><p id="agent-status" role="status" aria-live="polite"></p><p id="agent-error" class="form-error" role="alert"></p>`;
settings.querySelector('.modal-actions').before(section);
const $ = id => section.querySelector(`#${id}`);
let request = 0, pending = null, busy = false;
function text(tag, value, className) { const node = document.createElement(tag); node.textContent = value; if (className) node.className = className; return node; }
function option(value, label) { const node = text('option',label); node.value = value; return node; }
function clearPrivate() { $('agent-configuration').value = ''; $('agent-configuration').hidden = true; $('agent-export-consent').checked = false; }
function paint(snapshot) {
  if (!Array.isArray(snapshot?.clients) || !Array.isArray(snapshot?.apps)) throw new Error('Agent connections could not be read.');
  $('agent-panel').hidden = false;
  $('agent-boundary').textContent = snapshot.same_user_limitation;
  $('agent-enroll-form').querySelector('button').disabled = !snapshot.sidecar_available;
  $('agent-client-name').disabled = !snapshot.sidecar_available;
  $('agent-status').textContent = snapshot.sidecar_available ? '' : 'The agent connector is not installed in this build.';
  $('agent-grant-client').replaceChildren(...snapshot.clients.map(id => option(id,id)));
  $('agent-grant-app').replaceChildren(...snapshot.apps.map(app => option(app.id,`${app.display_name}${app.managed ? '' : ' · linked; status only'}`)));
  $('agent-grant-submit').disabled = !snapshot.clients.length || !snapshot.apps.length;
  $('agent-clients').replaceChildren(...snapshot.clients.map(id => {
    const row = text('div','', 'settings-row'); const name = text('strong',id); const revoke = text('button','Review removal','text-button'); revoke.type = 'button';
    revoke.addEventListener('click', () => {
      const review = text('span',`Remove ${id} and its app permissions? `);
      const confirm = text('button','Remove connection','danger'); confirm.type='button'; const cancel=text('button','Keep connection','text-button'); cancel.type='button';
      confirm.addEventListener('click', () => run(async () => { await invoke('agent_client_revoke',{clientId:id,consent:true}); if(pending?.client_id===id){pending=null;clearPrivate();$('agent-preview').hidden=true;} },'Connection removed.'));
      cancel.addEventListener('click', refresh); row.replaceChildren(review,confirm,cancel); confirm.focus();
    }); row.append(name,revoke); return row;
  }));
  $('agent-grants').replaceChildren(...snapshot.grants.map(grant => {
    const row=text('div','', 'settings-row'); row.append(text('span',`${grant.client_id} · ${grant.app_id}: ${grant.actions.join(', ')} · ${grant.expired ? 'expired' : new Date(grant.expires_at_unix*1000).toLocaleString()}${grant.installed ? '' : ' · app unavailable'}`));
    const revoke=text('button','Revoke permission','text-button');revoke.type='button';revoke.addEventListener('click',()=>run(()=>invoke('agent_grant_revoke',{clientId:grant.client_id,appId:grant.app_id}),'Permission revoked.'));row.append(revoke);return row;
  }));
  if (!snapshot.grants.length) $('agent-grants').append(text('p','No app permissions yet.','field-hint'));
  $('agent-audit').replaceChildren(...snapshot.audit.slice(0,10).map(event => text('p',`${event.client_id} · ${event.app_id} · ${event.action}: ${event.allowed ? 'allowed' : 'denied'} · ${new Date(event.at_unix*1000).toLocaleString()}`,'field-hint')));
  if (!snapshot.audit.length) $('agent-audit').append(text('p','No agent requests recorded.','field-hint'));
}
async function refresh() {
  const token=++request; $('agent-error').textContent=''; $('agent-refresh').disabled=true;
  try { const [snapshot,requests]=await Promise.all([invoke('agent_connections'),invoke('agent_mutation_requests')]); if(token===request&&settings.open){paint(snapshot);paintRequests(requests); } }
  catch(error){if(token===request&&settings.open)$('agent-error').textContent=error.message;}
  finally{if(token===request)$('agent-refresh').disabled=false;}
}
export {refresh as refreshAgents};
function paintRequests(requests) {
  if(!Array.isArray(requests)){$('agent-requests').textContent='Approval requests could not be read. Check again before approving anything.';return;}
  $('agent-requests').replaceChildren(...requests.slice(0,20).map(request=>{
    const card=text('div','', 'agent-request');const action=request.target.kind==='install'?'Install':'Uninstall and keep data';
    card.append(text('strong',`${request.client_id}: ${action} ${request.target.display_name}`),text('p',`Status: ${request.state}${request.stage ? ` · ${request.stage.replaceAll('_',' ')}` : ''}`,'field-hint'));
    if(request.state==='pending'){
      const label=text('label','', 'delete-choice');const consent=document.createElement('input');consent.type='checkbox';label.append(consent,text('span',`Allow ${request.client_id} to ${action.toLowerCase()} this app once.`));
      const allow=text('button','Approve once','secondary');allow.type='button';allow.disabled=true;consent.addEventListener('change',()=>allow.disabled=!consent.checked||busy);
      allow.addEventListener('click',()=>run(()=>invoke('agent_mutation_decide',{requestId:request.id,approve:true,consent:consent.checked}),'Request approved for 10 minutes.'));
      const deny=text('button','Deny request','text-button');deny.type='button';deny.addEventListener('click',()=>run(()=>invoke('agent_mutation_decide',{requestId:request.id,approve:false,consent:true}),'Request denied.'));
      card.append(label,allow,deny);
    }
    if(request.state==='approved')card.append(text('p',`Approved until ${new Date(request.approved_until_unix*1000).toLocaleString()}. The agent must execute this exact request.`,'field-hint'));
    if(request.state==='failed')card.append(text('p','The action did not finish. Review app status and retained setup before retrying.','field-hint'));
    if(request.state==='running')card.append(text('p','If the agent connector closed unexpectedly, check the app and retained setup before retrying. A consumed approval cannot be run again.','field-hint'));
    return card;
  }));
  if(!requests.length)$('agent-requests').append(text('p','No requests waiting for review.','field-hint'));
}
async function run(action,success) {
  if(busy)return;busy=true;const token=++request;section.setAttribute('aria-busy','true');$('agent-error').textContent='';
  const buttons=[...section.querySelectorAll('button')].map(node=>[node,node.disabled]);buttons.forEach(([node])=>node.disabled=true);
  try{await action();if(token===request&&settings.open){await refresh();$('agent-status').textContent=success;}}
  catch(error){if(token===request&&settings.open)$('agent-error').textContent=error.message;}
  finally{busy=false;section.setAttribute('aria-busy','false');buttons.forEach(([node,disabled])=>{if(node.isConnected&&node.id!=='agent-grant-submit')node.disabled=disabled;});}
}
$('agent-refresh').addEventListener('click',refresh);
$('agent-enroll-form').addEventListener('submit',event=>{event.preventDefault();run(async()=>{
  pending=await invoke('agent_enrollment_begin',{clientId:$('agent-client-name').value.trim(),consent:$('agent-enroll-consent').checked,rotate:false});
  if(!settings.open){await invoke('agent_enrollment_cancel',{enrollmentId:pending.enrollment_id});pending=null;return;}
  clearPrivate();$('agent-preview').hidden=false;$('agent-preview-note').textContent=`${pending.client_id} has no app access. Configuration preview expires in 10 minutes.`;$('agent-export').hidden=false;$('agent-cancel').hidden=false;$('agent-enroll-consent').checked=false;
},'Review the connection before exporting.');});
$('agent-export').addEventListener('click',()=>run(async()=>{
  if(!pending)throw new Error('Create a connection first.');
  const exported=await invoke('agent_enrollment_export',{enrollmentId:pending.enrollment_id,consent:$('agent-export-consent').checked});
  if(!settings.open){pending=null;clearPrivate();return;}
  $('agent-configuration').value=JSON.stringify(exported.configuration,null,2);$('agent-configuration').hidden=false;$('agent-export').hidden=true;$('agent-cancel').hidden=true;pending=null;
},'Copy this private configuration into your agent. It will be cleared when Settings closes.'));
$('agent-cancel').addEventListener('click',()=>run(async()=>{if(pending)await invoke('agent_enrollment_cancel',{enrollmentId:pending.enrollment_id});pending=null;clearPrivate();$('agent-preview').hidden=true;},'Connection cancelled.'));
$('agent-grant-scope').addEventListener('change',()=>{$('agent-grant-hours').max=$('agent-grant-scope').value==='lifecycle'?'24':'720';$('agent-grant-hours').value=Math.min(Number($('agent-grant-hours').value)||1,Number($('agent-grant-hours').max));});
$('agent-grant-form').addEventListener('submit',event=>{event.preventDefault();run(async()=>{await invoke('agent_grant_set',{clientId:$('agent-grant-client').value,appId:$('agent-grant-app').value,scope:$('agent-grant-scope').value,hours:Number($('agent-grant-hours').value),consent:$('agent-grant-consent').checked});$('agent-grant-consent').checked=false;},'Permission saved.');});
settings.addEventListener('close',()=>{++request;clearPrivate();$('agent-panel').hidden=true;$('agent-preview').hidden=true;$('agent-refresh').disabled=false;$('agent-status').textContent='';$('agent-error').textContent='';if(pending){invoke('agent_enrollment_cancel',{enrollmentId:pending.enrollment_id}).catch(()=>{});pending=null;}});
