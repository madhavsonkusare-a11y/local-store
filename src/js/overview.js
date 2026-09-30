import {escapeHtml, avatar} from './render.js';

const $ = id => document.getElementById(id);

function engineMessage(status) {
  if (!status) return ['Not checked', 'Use Settings to inspect or repair the managed engine.', 'neutral'];
  if (status.prerequisites?.state !== 'ready') return ['Windows setup needed', 'The managed engine needs Windows Subsystem for Linux setup. Linked apps remain available.', 'warning'];
  if (status.bootstrap?.disposition === 'ready' && status.daemon === 'responsive') return ['Managed engine ready', 'The verified Local Store engine is responding.', 'success'];
  if (status.bootstrap?.disposition === 'ready' && status.daemon === 'unresponsive') return ['Managed engine needs attention', 'The verified engine is installed but is not responding. Review repair in Settings.', 'warning'];
  if (status.bootstrap?.status === 'not_configured') return ['Managed engine not configured', 'Set up the managed engine before installing local apps. Linked apps remain available.', 'neutral'];
  return ['Engine review needed', 'Open Settings to inspect the current engine state before managing local apps.', 'warning'];
}

export function renderOverview(apps, engine, appError, engineError) {
  const mount = $('overview-content');
  if (appError) {
    mount.innerHTML = `<div class="overview-panel overview-empty"><h2>Couldn’t load your apps.</h2><p>${escapeHtml(appError.message || String(appError))}</p><button class="primary" data-overview="refresh">Try again</button></div>`;
    return;
  }
  const managed = apps.filter(app => app.runtime?.kind === 'compose');
  const linked = apps.filter(app => app.runtime?.kind !== 'compose');
  // A saved linked address is not proof of reachability. Only current backend
  // app status and errors contribute to this attention count.
  const attention = apps.filter(app => app.status_error || (app.runtime?.kind === 'compose' && app.status !== 'running'));
  const [engineTitle, engineDetail, engineTone] = engineMessage(engine);
  mount.innerHTML = `<section class="overview-hero overview-panel ${engineTone}" aria-labelledby="overview-health-title">
      <div><p class="overview-kicker">LOCAL ENVIRONMENT</p><h2 id="overview-health-title">${escapeHtml(engineTitle)}</h2><p>${escapeHtml(engineError ? `Engine status check failed: ${engineError.message || String(engineError)}` : engineDetail)}</p></div>
      <button class="secondary" data-overview="settings">View settings</button>
    </section>
    <section class="overview-metrics" aria-label="Saved app summary">
      <article class="overview-panel"><strong>${apps.length}</strong><span>Saved apps</span><small>Records on this computer</small></article>
      <article class="overview-panel"><strong>${managed.length}</strong><span>Managed apps</span><small>Local Store runtime</small></article>
      <article class="overview-panel"><strong>${linked.length}</strong><span>Linked apps</span><small>Existing instances</small></article>
      <article class="overview-panel"><strong>${attention.length}</strong><span>Need attention</span><small>Current app status</small></article>
    </section>
    <section class="overview-panel overview-attention" aria-labelledby="overview-attention-title"><div class="overview-section-head"><h2 id="overview-attention-title">${attention.length ? 'Needs a look' : 'Your apps'}</h2><span>${attention.length ? `${attention.length} ${attention.length === 1 ? 'app' : 'apps'}` : 'No reported app errors'}</span></div>
      ${attention.length ? attention.map(app => `<div class="overview-app-row">${avatar(app.display_name, app.icon_path)}<div><strong>${escapeHtml(app.display_name)}</strong><p>${escapeHtml(app.status_error || `Managed app · ${app.status || 'status unknown'}`)}</p></div></div>`).join('') : `<p class="overview-muted">${apps.length ? 'No saved app currently reports a stopped runtime or error. Linked addresses have not been checked here.' : 'No apps saved yet. Find an app to install, or connect one you already run.'}</p>`}
      <div class="overview-actions"><button class="secondary" data-overview="discover">Discover apps</button><button class="primary" data-overview="apps">${attention.length ? 'Review My Apps' : 'Open My Apps'}</button></div>
    </section>`;
}
