import {invoke} from './api.js';

const dialog = document.getElementById('settings-dialog');
const check = document.getElementById('refresh-engine');
const repair = document.getElementById('repair-engine');
const output = document.getElementById('engine-output');
const error = document.getElementById('engine-error');
let request = 0;

const states = {
  ready: ['Ready', 'The managed engine is responding.'],
  unresponsive: ['Needs attention', 'The managed engine is installed, but Docker is not responding. Repair starts Docker inside the verified Local Store engine.'],
  not_configured: ['Not configured', 'The managed engine has not been set up on this computer. Existing linked apps are unaffected.'],
  setup_required: ['Windows setup required', 'Windows Subsystem for Linux needs setup before the managed engine can run. Existing linked apps are unaffected.'],
  retry_import: ['Setup interrupted', 'The engine import can be retried after its files are reviewed. Nothing was changed by this check.'],
  resume_verification: ['Verification pending', 'The engine setup needs verification before it can run apps. Nothing was changed by this check.'],
  manual_review: ['Manual review needed', 'Engine ownership could not be verified. Local Store will not repair or remove this engine.'],
  unchecked: ['Not checked', 'The managed engine has not been checked yet. Check again before managing apps.'],
};

function stage(status) {
  if (status.bootstrap?.status === 'recovery' && status.bootstrap.disposition === 'manual_review') return 'manual_review';
  if (status.prerequisites?.state !== 'ready') return 'setup_required';
  if (status.bootstrap?.status === 'not_configured') return 'not_configured';
  if (status.bootstrap?.disposition === 'retry_import') return 'retry_import';
  if (status.bootstrap?.disposition === 'resume_verification') return 'resume_verification';
  if (status.bootstrap?.disposition === 'ready') {
    if (status.daemon === 'responsive') return 'ready';
    if (status.daemon === 'unresponsive') return 'unresponsive';
  }
  return 'unchecked';
}

function paint(status) {
  const kind = stage(status);
  const [title, description] = states[kind];
  const heading = document.createElement('strong');
  heading.className = `engine-state engine-state-${kind}`;
  heading.textContent = title;
  const detail = document.createElement('p');
  detail.textContent = description;
  output.replaceChildren(heading, detail);
  repair.hidden = kind !== 'unresponsive';
}

export async function refreshEngineStatus() {
  const token = ++request;
  check.disabled = true;
  check.textContent = 'Checking…';
  repair.hidden = true;
  output.textContent = '';
  error.textContent = '';
  try {
    const status = await invoke('managed_engine_status');
    if (token === request && dialog.open) {
      if (!status || typeof status !== 'object') throw new Error('Engine status could not be checked.');
      paint(status);
    }
  } catch (failure) {
    if (token === request && dialog.open) error.textContent = failure.message || 'Engine status could not be checked.';
  } finally {
    if (token === request) { check.disabled = false; check.textContent = 'Check again'; }
  }
}

check.addEventListener('click', refreshEngineStatus);
repair.addEventListener('click', async () => {
  const token = ++request;
  repair.disabled = true;
  check.disabled = true;
  repair.textContent = 'Repairing…';
  error.textContent = '';
  try {
    await invoke('repair_managed_engine');
    if (token === request && dialog.open) {
      repair.disabled = false;
      await refreshEngineStatus();
    }
  } catch (failure) {
    if (token === request && dialog.open) error.textContent = failure.message || 'The managed engine could not be repaired.';
  } finally {
    if (token === request) { repair.disabled = false; repair.textContent = 'Repair managed engine'; check.disabled = false; }
  }
});
dialog.addEventListener('close', () => {
  request++;
  check.disabled = false;
  check.textContent = 'Check status';
  repair.disabled = false;
  repair.textContent = 'Repair managed engine';
  repair.hidden = true;
});
