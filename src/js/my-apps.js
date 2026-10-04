import {loadingComposition} from './loading-composition.js';
import {avatar, escapeHtml, installedControls} from './render.js';

function date(value) {
  if (!Number.isFinite(value) || value <= 0) return 'Not recorded';
  return new Intl.DateTimeFormat('en', {year:'numeric', month:'short', day:'numeric'}).format(new Date(value * 1000));
}

function fact(label, value) {
  return `<div><dt>${label}</dt><dd>${escapeHtml(value ?? 'Not recorded')}</dd></div>`;
}

export function myAppsDetail(app, tab = 'overview', log = {state:'idle'}, index = -1) {
  if (!app) return '<div class="my-apps-detail-empty"><h2>Select an app</h2><p>Choose a saved app to inspect its address, status, and available controls.</p></div>';
  const managed = app.runtime?.kind === 'compose';
  const type = managed ? 'Managed app · Compose' : 'Linked app · External server';
  const status = managed ? app.status || 'not checked' : 'address not checked';
  const tabs = [['overview','Overview'], ...(managed ? [['logs','Logs']] : []), ['manage','Manage']];
  const body = tab === 'logs' && managed
    ? `<div class="my-apps-logs-head"><p>Recent Compose output · fetched on request</p><button class="secondary" data-my-apps-log="refresh" ${log.state === 'loading' ? 'disabled' : ''}>${log.state === 'loading' ? 'Loading…' : 'Refresh logs'}</button>${log.state === 'ready' ? `<button class="secondary" data-my-apps-log="copy">Copy logs</button>` : ''}</div><p role="status" aria-live="polite">${escapeHtml(log.copyStatus || '')}</p>${log.state === 'error' ? `<p class="form-error" role="alert">${escapeHtml(log.error)}</p>` : log.state === 'ready' ? `<pre class="log-view" tabindex="0" aria-label="Recent logs for ${escapeHtml(app.display_name)}">${escapeHtml(log.text || 'No recent output.')}</pre>` : log.state === 'loading' ? loadingComposition('Loading recent output…', 'logs') : '<p class="overview-muted">Logs have not been requested in this session.</p>'}`
    : tab === 'manage'
      ? `<div class="my-apps-manage"><h3>${managed ? 'Local Store manages this app' : 'This is a saved connection'}</h3><p>${managed ? 'Open, start, stop, inspect logs, or review uninstall. Uninstall keeps managed data by default.' : 'Open the address, create a shortcut, or remove this record. The external server and its data stay under your control.'}</p>${index >= 0 ? `<div class="installed-actions">${installedControls(app,index)}</div>` : ''}</div>`
      : `<div class="my-apps-status"><strong id="my-apps-detail-status">${escapeHtml(status)}</strong><p>${managed ? 'Container state reported by the local runtime. Opening the app is a separate readiness check.' : 'Local Store stores this address. It does not start or manage the external server.'}</p></div><dl class="my-apps-facts">${fact('Address', app.launch_url)}${fact('Runs as', type)}${fact('Catalog match', app.catalog_id || 'Not linked to catalog')}${fact('Added', date(app.created_at_unix))}${fact('Last changed', date(app.updated_at_unix))}</dl>${managed && (app.runtime.project_dir || app.runtime.compose_file) ? `<details class="my-apps-paths"><summary>Managed project paths</summary><dl>${app.runtime.project_dir ? fact('Project folder', app.runtime.project_dir) : ''}${app.runtime.compose_file ? fact('Compose file', app.runtime.compose_file) : ''}</dl></details>` : ''}`;
  return `<div class="my-apps-detail-head" data-app-id="${escapeHtml(app.id)}">${avatar(app.display_name, app.icon_path)}<div><p class="eyebrow">${managed ? 'MANAGED APP' : 'LINKED APP'}</p><h2 id="my-apps-detail-title">${escapeHtml(app.display_name)}</h2><p>${escapeHtml(type)}</p></div>${tab !== 'manage' && index >= 0 ? `<div class="installed-actions">${installedControls(app,index)}</div>` : ''}</div><div class="my-apps-tabs" role="tablist" aria-label="${escapeHtml(app.display_name)} details">${tabs.map(([key,label]) => `<button id="my-apps-tab-${key}" type="button" role="tab" data-my-apps-tab="${key}" aria-selected="${tab === key}" aria-controls="my-apps-panel" tabindex="${tab === key ? '0' : '-1'}">${label}</button>`).join('')}</div><div id="my-apps-panel" data-app-id="${escapeHtml(app.id)}" class="my-apps-detail-body" role="tabpanel" aria-labelledby="my-apps-tab-${tab}">${body}</div>`;
}
