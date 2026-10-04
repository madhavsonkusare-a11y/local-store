import {invoke} from './api.js';

let revision = 0;
export function publishEngineStatus(status, failure = false) {
  revision++;
  const card = document.getElementById('rail-engine');
  if (!card) return;
  let label = 'Not checked', tone = 'neutral', detail = 'View settings';
  if (failure) { label = 'Check failed'; detail = 'Retry in Settings'; tone = 'warning'; }
  else if (status) {
    const bootstrap = status.bootstrap || {};
    if (bootstrap.disposition === 'manual_review') { label = 'Manual review'; tone = 'warning'; }
    else if (status.prerequisites?.state !== 'ready') { label = 'Windows setup needed'; tone = 'warning'; }
    else if (bootstrap.disposition === 'ready' && status.daemon === 'responsive') { label = 'Engine ready'; tone = 'success'; }
    else if (bootstrap.disposition === 'ready' && status.daemon === 'unresponsive') { label = 'Needs attention'; tone = 'warning'; }
    else if (bootstrap.status === 'not_configured') label = 'Not configured';
    else if (bootstrap.disposition === 'retry_import') { label = 'Setup interrupted'; tone = 'warning'; }
    else if (bootstrap.disposition === 'resume_verification') { label = 'Verification pending'; tone = 'warning'; }
    detail = status.daemon && status.daemon !== 'not_checked' ? `Daemon · ${status.daemon}` : 'View settings';
  }
  card.dataset.tone = tone;
  document.getElementById('rail-engine-state').textContent = label;
  document.getElementById('rail-engine-detail').textContent = detail;
  card.setAttribute('aria-label', `Managed engine: ${label}. View Settings`);
  card.setAttribute('aria-busy', 'false');
}

export async function refreshRailEngine() {
  const token = revision;
  const card = document.getElementById('rail-engine');
  card.setAttribute('aria-busy', 'true');
  document.getElementById('rail-engine-state').textContent = 'Checking…';
  try { const report = await invoke('managed_engine_status'); if (token === revision) publishEngineStatus(report); }
  catch { if (token === revision) publishEngineStatus(null, true); }
}
