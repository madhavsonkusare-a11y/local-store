import {invoke} from './api.js';
const ids = ['flatnotes','gitea','immich','jellyfin','kanboard','memos','n8n','privatebin','uptime-kuma','wordpress'];
const section = document.createElement('section'); section.className = 'launch-collection'; section.setAttribute('aria-label', 'Windows launch collection');
const heading = document.createElement('h3'); heading.textContent = 'Windows launch collection';
const summary = document.createElement('p'); summary.id = 'launch-collection-summary'; summary.setAttribute('role','status');
const detail = document.createElement('p'); detail.className = 'field-hint'; detail.textContent = 'These counts describe installation and app-task proof. Agent content permissions are reviewed separately. Additional catalog apps remain install previews.';
section.append(heading, summary, detail);
document.getElementById('settings-dialog').querySelector('.modal-actions').before(section);
let generation = 0;
export async function refreshLaunchCollection() {
  const current = ++generation;
  summary.textContent = 'Checking current launch proof…';
  try {
    const facts = await invoke('launch_readiness_batch', {ids});
    if (current !== generation) return;
    if (!Array.isArray(facts) || facts.length !== ids.length || !ids.every(id => facts.filter(fact => fact.offering_id === id).length === 1)) throw new Error('Incomplete launch proof');
    const zero = facts.filter(fact => fact.install_mode === 'zero_input').length;
    const setup = facts.filter(fact => fact.install_mode === 'setup_assisted').length;
    const tasks = facts.filter(fact => fact.task_verified === true && fact.current_evidence === true).length;
    summary.textContent = `${ids.length} selected apps · ${zero} zero-input installs · ${setup} setup-assisted installs · ${tasks} current verified tasks`;
    if (zero + setup !== ids.length) summary.textContent += ` · ${ids.length-zero-setup} unavailable for installation`;
  } catch {
    if (current === generation) summary.textContent = 'Launch proof could not be checked. No verified count is available.';
  }
}
