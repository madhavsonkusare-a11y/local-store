import {invoke} from './api.js';

const settings = document.getElementById('settings-dialog');
const section = document.createElement('section');
section.className = 'agent-settings';
section.innerHTML = `<div class="settings-row"><div><h3>App content access</h3><p>Connect reviewed apps so an agent can read content and request specific writes.</p></div><button class="secondary" id="content-refresh">Manage app access</button></div>
<div id="content-panel" hidden><p class="field-hint">Memos supports private notes. n8n supports workflow summaries and a fixed note table through its own MCP server. PrivateBin uses a separate isolated browser for pastes created through this connection. Gitea, WordPress, Kanboard, Immich, Jellyfin and Uptime Kuma provide reviewed read-only summaries. Flatnotes file access has its own separate permission below. Reconnecting or disconnecting an app clears its earlier agent permissions; grant access again after connecting. Permissions apply to this connection; an agent with unrestricted access to your Windows account can bypass them.</p>
<form id="content-connect-form"><label class="field">Installed supported app<select id="content-app" required></select></label><label class="field">App access token<input id="content-token" type="password" minlength="16" maxlength="8192" required autocomplete="off" spellcheck="false"></label>
<p id="content-connect-help" class="field-hint">Create an access token in the selected app. n8n also requires its instance MCP server to be enabled in Settings → MCP access; paste that server's token.</p>
<label class="delete-choice"><input id="content-connect-consent" type="checkbox" required><span id="content-connect-consent-copy">Store this app token protected by my Windows account. This does not grant an agent access.</span></label><button class="secondary" id="content-connect-submit" type="submit">Connect app</button></form>
<div id="content-connections"></div><form id="content-grant-form"><h3>Content permissions</h3><label class="field">Agent connection<select id="content-client" required></select></label><label class="field">Connected app<select id="content-grant-app" required></select></label>
<label class="field">Content actions<select id="content-scope"><option value="read">Read supported app content</option><option value="write">Read content and request reviewed writes</option></select></label><label class="field">Content permission expires after (hours)<input id="content-hours" type="number" min="1" max="24" value="1" required></label>
<label class="delete-choice"><input id="content-grant-consent" type="checkbox" required><span>Allow these content actions for this exact app until expiry. Each write still needs my separate approval.</span></label><button class="secondary" id="content-grant-submit" type="submit">Save content permission</button></form>
<p class="field-hint">Recovery is limited to consistent local backups. A backup cannot undo messages, external service changes or other effects outside the app. Snapshot and restore support has been proven for Memos and Gitea; it is not automatic undo for every agent action.</p>
<h3>App write requests</h3><p class="field-hint">Review the exact details. Approval lasts up to two minutes and can perform this write once. App content cannot change permissions.</p><div id="content-requests"></div></div><p id="content-status" role="status" aria-live="polite"></p><p id="content-error" class="form-error" role="alert"></p>`;
settings.querySelector('.modal-actions').before(section);
const $ = id => section.querySelector(`#${id}`);
let busy = false, revision = 0;
let appKinds = new Map(), readOnlyApps = new Set();
function node(tag, value, className) { const element = document.createElement(tag); element.textContent = value; if (className) element.className = className; return element; }
function option(value, label) { const element = node('option', label); element.value = value; return element; }
function clearPrivate() { $('content-token').value = ''; $('content-connect-consent').checked = false; $('content-requests').replaceChildren(); }
function appChoice() {
  const kind = appKinds.get($('content-app').value) || $('content-app').value;
  const browser = kind === 'privatebin';
  $('content-token').required = !browser; $('content-token').closest('label').hidden = browser;
  $('content-token').value = ''; $('content-connect-consent').checked = false;
  $('content-connect-help').textContent = browser ? 'Uses a pinned isolated browser with no network access. The first connection may download its large browser image. Only pastes created through this connection can be read; arbitrary historical paste links are not accepted.' : kind === 'n8n' ? 'Enable the instance MCP server in n8n Settings → MCP access and paste its server token. Workflow detail reads also require you to expose the exact workflow in n8n; Local Store never enables that automatically.' : kind === 'kanboard' ? 'Use your Kanboard username:user API token. The global JSON-RPC token is not supported. This provider reads your dashboard only.' : kind === 'wordpress' ? 'Use your WordPress username:password for fixed XML-RPC reads of your permitted posts. No post writes or arbitrary XML-RPC methods are available.' : kind === 'jellyfin' ? 'Use your Jellyfin app-issued API or user token. This provider reads media identities only; no paths or playback links are returned.' : kind === 'uptime-kuma' ? 'Use your Uptime Kuma username:password or the app’s supported username:API key credentials. Authenticated metrics must be enabled. This provider returns monitor status summaries only; target addresses and tags are excluded.' : 'Create an app-issued access token or scoped API key for the selected app. Its read scope determines which content can be returned.';
  $('content-connect-consent-copy').textContent = browser ? 'Prepare the isolated app browser and protect its paste keys with my Windows account. This does not grant an agent access.' : 'Store this app token protected by my Windows account. This does not grant an agent access.';
}
function grantChoice() {
  const readOnly = readOnlyApps.has($('content-grant-app').value);
  $('content-scope').querySelector('option[value="write"]').disabled = readOnly;
  if (readOnly) $('content-scope').value = 'read';
}
function paint(connections, snapshot, requests) {
  if (!Array.isArray(connections) || !Array.isArray(snapshot?.apps) || !Array.isArray(snapshot?.clients) || !Array.isArray(requests)) throw new Error('App access could not be read.');
  $('content-panel').hidden = false;
  const supported = ['memos','n8n','privatebin','gitea','wordpress','kanboard','immich','jellyfin','uptime-kuma'];
  const apps = snapshot.apps.filter(app => app.managed && (supported.includes(app.id) || supported.includes(app.catalog_id)));
  appKinds = new Map(apps.map(app => [app.id, app.catalog_id || app.id]));
  readOnlyApps = new Set(apps.filter(app => ['gitea','wordpress','kanboard','immich','jellyfin','uptime-kuma'].includes(app.catalog_id || app.id)).map(app => app.id));
  $('content-app').replaceChildren(...apps.map(app => option(app.id, app.display_name)));
  appChoice();
  $('content-connect-submit').disabled = !apps.length;
  $('content-client').replaceChildren(...snapshot.clients.map(client => option(client, client)));
  $('content-grant-app').replaceChildren(...connections.filter(connection => apps.some(app => app.id === connection.app_id)).map(connection => option(connection.app_id, connection.app_id)));
  grantChoice();
  $('content-grant-submit').disabled = !snapshot.clients.length || !$('content-grant-app').options.length;
  $('content-connections').replaceChildren(...connections.map(connection => {
    const row = node('div', '', 'settings-row'); row.append(node('span', `${connection.app_id} · token protected by your Windows account`));
    const remove = node('button', 'Review disconnection', 'text-button'); remove.type = 'button';
    remove.addEventListener('click', () => {
      const confirm = node('button', 'Disconnect app content', 'danger'); confirm.type = 'button';
      confirm.addEventListener('click', () => run(() => invoke('agent_content_disconnect', {appId: connection.app_id, consent: true}), 'App content disconnected. Pending app approvals were removed.'));
      const cancel = node('button', 'Keep app connection', 'text-button'); cancel.type = 'button'; cancel.addEventListener('click', refreshContentAccess);
      const consequence = appKinds.get(connection.app_id) === 'privatebin' ? 'This removes its protected fragment keys. Encrypted pastes remain until expiry, but this connection can no longer decrypt them.' : 'Agents will lose its content connection.';
      row.replaceChildren(node('span', `Disconnect ${connection.app_id}? ${consequence}`), confirm, cancel); confirm.focus();
    }); row.append(remove); return row;
  }));
  $('content-requests').replaceChildren(...requests.slice(0,32).map(request => {
    const table = request.operation_description === 'Create n8n note table';
    const paste = request.operation_description === 'Create encrypted PrivateBin paste';
    const operation = table ? 'Create n8n note table' : paste ? 'Create encrypted PrivateBin paste' : 'Create private memo';
    const card = node('div', '', 'agent-request'); card.append(node('strong', `${request.client_id} · ${request.app_id} · ${operation}`));
    const label = node('label', table ? 'Exact requested table details' : paste ? 'Exact requested paste text' : 'Exact requested memo text', 'field'); const content = document.createElement('textarea'); content.readOnly = true; content.value = request.content; content.className = 'log-view'; content.spellcheck = false; label.append(content); card.append(label);
    if (table) card.append(node('p', 'Creates one table with a single note text column. No workflow execution, code, credential changes or deletion.', 'field-hint'));
    if (paste) card.append(node('p','Creates a non-burning encrypted paste with one-day expiry and no discussion. Its fragment key remains protected inside this connection. No deletion or arbitrary links.','field-hint'));
    if (request.approved_until_unix) { card.append(node('p', `Approved once until ${new Date(request.approved_until_unix*1000).toLocaleString()}.`, 'field-hint')); return card; }
    const consent = node('label', '', 'delete-choice'); const check = document.createElement('input'); check.type = 'checkbox'; consent.append(check,node('span', `Allow ${request.client_id} to create this exact ${table ? 'note table' : paste ? 'encrypted paste' : 'private memo'} once.`));
    const allow = node('button', table ? 'Approve table once' : paste ? 'Approve paste once' : 'Approve memo once', 'secondary'); allow.type = 'button'; allow.disabled = true; check.addEventListener('change', () => { allow.disabled = !check.checked || busy; });
    allow.addEventListener('click', () => run(() => invoke('agent_content_decide', {requestId: request.id, approve: true, consent: check.checked}), 'Exact app write approved once.'));
    const deny = node('button', table ? 'Deny table' : paste ? 'Deny paste' : 'Deny memo', 'text-button'); deny.type = 'button'; deny.addEventListener('click', () => run(() => invoke('agent_content_decide', {requestId: request.id, approve: false, consent: true}), 'App write request denied.'));
    card.append(consent, allow, deny); return card;
  }));
  if (!requests.length) $('content-requests').append(node('p', 'No app write requests waiting for review.', 'field-hint'));
}
export async function refreshContentAccess() {
  const token = ++revision; $('content-error').textContent = ''; $('content-refresh').disabled = true;
  try { const [connections,snapshot,requests] = await Promise.all([invoke('agent_content_connections'),invoke('agent_connections'),invoke('agent_content_requests')]); if (token === revision && settings.open) paint(connections,snapshot,requests); }
  catch { if (token === revision && settings.open) $('content-error').textContent = 'App access could not be read. Check again before approving a request.'; }
  finally { if (token === revision) $('content-refresh').disabled = false; }
}
async function run(action, success) {
  if (busy) return; busy = true; const token = ++revision; section.setAttribute('aria-busy','true'); $('content-error').textContent = '';
  const buttons = [...section.querySelectorAll('button')].map(button => [button,button.disabled]); buttons.forEach(([button]) => { button.disabled = true; });
  try { await action(); if (token === revision && settings.open) { await refreshContentAccess(); $('content-status').textContent = success; } }
  catch { if (token === revision && settings.open) $('content-error').textContent = 'Could not update app access. Check the running app, token, client and permission expiry. No automatic retry was made.'; }
  finally { busy = false; section.setAttribute('aria-busy','false'); buttons.forEach(([button,disabled]) => { if (button.isConnected && !['content-connect-submit','content-grant-submit'].includes(button.id)) button.disabled = disabled; }); }
}
$('content-refresh').addEventListener('click', refreshContentAccess);
$('content-app').addEventListener('change', appChoice);
$('content-grant-app').addEventListener('change', grantChoice);
$('content-connect-form').addEventListener('submit', event => {
  event.preventDefault(); if (busy) return;
  const args = {appId:$('content-app').value, token:$('content-token').value, consent:$('content-connect-consent').checked};
  $('content-token').value = ''; $('content-connect-consent').checked = false;
  run(() => invoke('agent_content_connect', args), 'App connected. Review an agent permission separately.');
});
$('content-grant-form').addEventListener('submit', event => { event.preventDefault(); run(async () => { await invoke('agent_content_grant', {clientId:$('content-client').value, appId:$('content-grant-app').value, hours:Number($('content-hours').value), write:$('content-scope').value === 'write', consent:$('content-grant-consent').checked}); $('content-grant-consent').checked = false; }, 'Content permission saved. Every write requires a separate exact-text approval.'); });
settings.addEventListener('close', () => { ++revision; clearPrivate(); $('content-panel').hidden = true; $('content-status').textContent = ''; $('content-error').textContent = ''; $('content-refresh').disabled = false; });
