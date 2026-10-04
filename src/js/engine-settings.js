import {publishEngineStatus} from './rail-engine.js';
import {loadingComposition} from './loading-composition.js';
import {invoke} from './api.js';
import {setDialogBusy} from './motion.js';

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
  output.innerHTML = loadingComposition('Checking the managed engine…');
  error.textContent = '';
  try {
    const [status, preview] = await Promise.all([invoke('managed_engine_status'), invoke('engine_setup_preview').catch(() => null)]);
    if (token === request && dialog.open) {
      if (!status || typeof status !== 'object') throw new Error('Engine status could not be checked.');
      paint(status);
      publishEngineStatus(status);
      if (preview?.engine) paintSetup(preview);
    }
  } catch (failure) {
    if (token === request && dialog.open) { output.replaceChildren(); publishEngineStatus(null, true); error.textContent = failure.message || 'Engine status could not be checked.'; }
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

function paintSetup(preview) {
  const summary = document.createElement('p');
  const gib = bytes => (bytes / 1073741824).toFixed(1);
  summary.textContent = `Local Store engine only. Docker Desktop is not required. Setup checks a ${gib(preview.required_disk_bytes)} GB free-space budget. Apps and their images need additional space.`;
  output.append(summary);
  if (preview.payload !== 'verified_development' && !preview.development_engine_available && preview.engine.bootstrap?.status === 'not_configured') {
    const missing = document.createElement('p'); missing.className = 'warning-label';
    missing.textContent = 'A verified bundled engine payload is unavailable. Managed installation is blocked until the payload is provided; linked apps still work.';
    output.append(missing); return;
  }
  const kind = stage(preview.engine);
  let action = null, label = null;
  if (preview.development_engine_available && kind === 'not_configured') { action = 'adopt_development'; label = 'Use existing Local Store engine'; }
  else if (kind === 'not_configured') { action = 'install'; label = 'Set up Local Store engine'; }
  else if (kind === 'retry_import') { action = 'retry_import'; label = 'Retry local engine setup'; }
  else if (kind === 'resume_verification') { action = 'resume_verification'; label = 'Finish local engine verification'; }
  else if (kind === 'ready' && !preview.selected_engine) { action = 'select_managed'; label = 'Use Local Store engine for installs'; }
  if (kind === 'setup_required') {
    const prerequisite = document.createElement('p'); prerequisite.className = 'field-hint';
    prerequisite.textContent = 'Enable Windows Subsystem for Linux and Virtual Machine Platform in Windows Features, complete any administrator prompt, then restart Windows if requested. Check status again afterward. No engine was created by this check.';
    output.append(prerequisite);
  }
  if (!action) return;
  if (!preview.payload_release_approved) {
    const development = document.createElement('p'); development.className = 'field-hint';
    development.textContent = 'This is a development engine for personal use. Its payload is not approved for a packaged Windows release.';
    output.append(development);
  }
  if (preview.setup_requires_administrator) { const prerequisite = document.createElement('p'); prerequisite.className = 'field-hint'; prerequisite.textContent = 'Windows may request administrator permission for its Linux prerequisite. A restart may be needed. Local Store will not approve that prompt for you.'; output.append(prerequisite); }
  const consent = document.createElement('div'); consent.className = 'engine-consent';
  const choices = [
    ['create_owned_engine', action === 'adopt_development' ? 'Use the verified existing Local Store engine on this computer.' : 'Create or select the separate Local Store engine for managed apps.'],
    ['use_disk_space', `Allow the local engine to use disk space (at least ${gib(preview.required_disk_bytes)} GB for setup).`],
    ['acknowledge_existing_apps_unchanged', 'My existing apps and their data stay on their current engine. This does not migrate or delete them.'],
  ];
  for (const [key,text] of choices) { const row = document.createElement('label'); const input = document.createElement('input'); input.type = 'checkbox'; input.dataset.engineConsent = key; const wording = document.createElement('span'); wording.textContent = text; row.append(input,wording); consent.append(row); }
  const start = document.createElement('button'); start.className = 'primary'; start.id = 'engine-setup-start'; start.textContent = label;
  if (preview.disk_sufficient === false && ['install','retry_import'].includes(action)) { start.disabled = true; const low = document.createElement('p'); low.className = 'warning-label'; low.textContent = 'There is not enough disk space for engine setup. Free space and check again.'; output.append(low); }
  if (preview.disk_sufficient === null) { const unknown = document.createElement('p'); unknown.className = 'field-hint'; unknown.textContent = 'Available disk space could not be measured. Setup checks it again before importing.'; output.append(unknown); }
  start.onclick = async () => {
    const inputs = [...consent.querySelectorAll('input')];
    const missing = inputs.find(input => !input.checked);
    if (missing) { error.textContent = 'Review and accept each engine setup choice to continue.'; missing.focus(); return; }
    const agreed = Object.fromEntries(inputs.map(input => [input.dataset.engineConsent,input.checked]));
    start.disabled = true; check.disabled = true; inputs.forEach(input => {input.disabled = true;}); setDialogBusy(dialog,true);
    start.textContent = 'Preparing local engine…'; error.textContent = '';
    try {
      await invoke('engine_setup_action', {action,consent:agreed});
      setDialogBusy(dialog,false);
      await refreshEngineStatus();
    } catch (failure) { error.textContent = failure.message || 'Local engine setup did not finish. Check status before retrying.'; }
    finally { setDialogBusy(dialog,false); start.disabled = false; check.disabled = false; inputs.forEach(input => {input.disabled = false;}); start.textContent = label; }
  };
  output.append(consent,start);
}
