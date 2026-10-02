import {invoke} from './api.js';
import {showDialog, closeDialog, setDialogBusy} from './motion.js';
const $ = id => document.getElementById(id);
const button = $('scan-recovery'), output = $('recovery-output'), error = $('recovery-error');
const settings = $('settings-dialog'), task = $('recovery-task');
let request = 0, active = null, busy = false;
const reset = () => { button.disabled = false; button.textContent = 'Scan for setups'; };
settings.addEventListener('close', () => {request++; reset();});
function label(candidate) {
  return candidate.ownership_status === 'verified'
    ? 'Matching Docker setup found. Its ownership will be checked again before recovery.'
    : candidate.ownership_status === 'no_containers'
      ? 'Retained setup files found, with no matching container. These may be data you chose to keep.'
      : 'This setup could not be verified. Leave it unchanged until its ownership is confirmed.';
}
function canReview(candidate) { return Boolean(candidate.recipe_id) && (candidate.ownership_status === 'no_containers' || (candidate.ownership_status === 'verified' && candidate.docker_ownership_verified)); }
async function review(candidate) {
  active = candidate;
  $('recovery-title').textContent = `Recover ${candidate.display_name}`;
  $('recovery-description').textContent = 'Resume setup without replacing saved credentials, or stop the retained app while keeping its data. Ownership is checked again when you choose an action.';
  $('recovery-task-status').textContent = '';
  $('recovery-task-error').textContent = '';
  const notes = document.createElement('p'); notes.textContent = label(candidate); notes.className = 'field-hint';
  const location = document.createElement('p'); location.textContent = candidate.compose_file; location.className = 'recovery-path';
  const consent = document.createElement('label'); consent.className = 'delete-choice';
  const checkbox = document.createElement('input'); checkbox.type = 'checkbox'; checkbox.id = 'recovery-delete-data';
  const text = document.createElement('span'); text.textContent = 'Delete retained data too. This permanently removes the managed data and volumes.';
  consent.append(checkbox,text);
  const confirm = document.createElement('label'); confirm.className = 'field'; confirm.id = 'recovery-delete-confirm-field'; confirm.hidden = true;
  const hint = document.createElement('span'); hint.textContent = `To delete data, type ${candidate.display_name}`;
  const input = document.createElement('input'); input.id = 'recovery-delete-confirm'; input.autocomplete = 'off'; confirm.append(hint,input);
  checkbox.onchange = () => { confirm.hidden = !checkbox.checked; $('recovery-clear').textContent = checkbox.checked ? 'Delete setup and data' : 'Clear setup, keep data'; if (checkbox.checked) input.focus(); };
  $('recovery-review').replaceChildren(notes, location, consent, confirm);
  $('recovery-resume').hidden = false; $('recovery-clear').hidden = false;
  $('recovery-clear').textContent = 'Clear setup, keep data';
  await closeDialog(settings); await showDialog(task); $('recovery-title').focus();
}
async function scan() {
  const token = ++request; button.disabled = true; button.textContent = 'Scanning…'; output.replaceChildren(); error.textContent = '';
  try {
    const candidates = await invoke('inspect_recovery');
    if (token !== request || !settings.open) return;
    if (!candidates.length) { output.textContent = 'No retained setups found for supported apps.'; return; }
    for (const candidate of candidates) {
      const row = document.createElement('div'); row.className = 'settings-row';
      const content = document.createElement('div');
      const title = document.createElement('h3'); title.textContent = candidate.display_name;
      const description = document.createElement('p'); description.textContent = label(candidate);
      const details = document.createElement('details'); const summary = document.createElement('summary'); summary.textContent = 'Setup location';
      const path = document.createElement('p'); path.textContent = candidate.compose_file; path.style.overflowWrap = 'anywhere';
      details.append(summary,path); content.append(title,description,details); row.append(content);
      if (canReview(candidate)) { const action = document.createElement('button'); action.className = 'secondary'; action.textContent = 'Review setup'; action.onclick = () => review(candidate); row.append(action); }
      output.append(row);
    }
  } catch (failure) { if (token === request && settings.open) error.textContent = failure.message || 'Could not inspect setups. Check the local engine and try again.'; }
  finally { if (token === request) reset(); }
}
button.addEventListener('click',scan);
async function recover(kind) {
  if (!active || busy) return;
  const deleting = kind === 'clear' && $('recovery-delete-data').checked;
  if (deleting && $('recovery-delete-confirm').value !== active.display_name) {
    $('recovery-task-error').textContent = 'Type the app name exactly before deleting its data.';
    $('recovery-delete-confirm').setAttribute('aria-invalid','true'); $('recovery-delete-confirm').focus(); return;
  }
  busy = true; setDialogBusy(task,true); $('recovery-task-error').textContent = '';
  task.querySelectorAll('button,input').forEach(control => {control.disabled = true;});
  $('recovery-task-status').textContent = kind === 'resume' ? 'Checking ownership and resuming setup…' : 'Checking ownership and clearing setup…';
  $('recovery-task-status').focus();
  try {
    const result = await invoke(kind === 'resume' ? 'adopt_retained_setup' : 'discard_retained_setup', {recipeId:active.recipe_id, ...(kind === 'clear' ? {deleteData:deleting} : {})});
    $('recovery-task-status').textContent = kind === 'resume'
      ? `Setup recovered. ${result.containers} owned containers checked. Open ${active.display_name} from My Apps to finish any app setup.`
      : result.data_deleted ? 'Retained setup and managed data deleted.' : `Retained setup cleared. ${result.containers_removed} owned containers removed. Managed data was kept.`;
    $('recovery-resume').hidden = true; $('recovery-clear').hidden = true;
    $('recovery-review').replaceChildren();
    window.dispatchEvent(new CustomEvent('local-store:apps-changed'));
  } catch (failure) {
    $('recovery-task-status').textContent = 'Recovery did not finish. No success is assumed.';
    $('recovery-task-error').textContent = failure.message || 'Recovery could not finish. Review the retained setup and try again.';
  } finally {
    busy = false; setDialogBusy(task,false); task.querySelectorAll('button,input').forEach(control => {control.disabled = false;});
  }
}
$('recovery-resume').onclick = () => recover('resume'); $('recovery-clear').onclick = () => recover('clear');
task.addEventListener('close', () => { active = null; void showDialog(settings).then(() => {button.focus();}); });
