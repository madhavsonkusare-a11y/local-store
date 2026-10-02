import {escapeHtml} from './render.js';

export function readinessLabel(facts) {
  if (facts?.task_verified && facts?.current_evidence) return 'Task verified · Windows';
  if (facts?.lifecycle_proven && facts?.current_evidence) return 'Lifecycle verified · Windows';
  return 'Install preview';
}
export function readinessView(facts) {
  if (!facts) return '<p class="field-hint">Proof could not be checked. No verification claim is made.</p>';
  const install = {zero_input:'No required answers before install', setup_assisted:'Setup answers needed before install', discovery:'Discover or connect an existing app', setup_unavailable:'Setup requirements unavailable'}[facts.install_mode] || 'Review setup requirements';
  const task = facts.task_verified && facts.current_evidence;
  const lifecycle = facts.lifecycle_proven && facts.current_evidence;
  const content = {verified_read:'Read-only provider verified · separate permission required', verified_read_write:'Reads and reviewed writes verified · each write needs approval'}[facts.agent_content_access] || 'App content access is not verified';
  return `<section class="launch-readiness" aria-label="App capabilities"><h3>What is ready</h3><dl><div><dt>Installation</dt><dd>${escapeHtml(install)}</dd></div><div><dt>Windows lifecycle</dt><dd>${lifecycle ? 'Install, restart and keep-data reinstall verified' : 'Current lifecycle proof unavailable'}</dd></div><div><dt>App task</dt><dd>${task ? escapeHtml(facts.task || 'A useful task is verified') : 'A working page alone does not verify a useful task'}</dd></div><div><dt>Agent content</dt><dd>${escapeHtml(content)}</dd></div></dl>${facts.caution ? `<p class="warning-label">${escapeHtml(facts.caution)}</p>` : ''}${facts.prerequisites?.length ? `<h4>Before downloading</h4><ul class="risk-list">${facts.prerequisites.map(note => `<li>${escapeHtml(note)}</li>`).join('')}</ul>` : '<p class="field-hint">No additional required setup answers are declared. App accounts and permissions may still need setup after opening.</p>'}</section>`;
}
