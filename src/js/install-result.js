import {markStarterStep} from './first-run.js';
import {invoke} from './api.js';
import {showDialog, closeDialog, setDialogBusy} from './motion.js';

const dialog = document.createElement('dialog');
dialog.id = 'install-result-dialog';
dialog.className = 'wide-dialog focused-task install-result';
dialog.dataset.parentNav = 'nav-apps';
dialog.setAttribute('aria-labelledby', 'install-result-title');
dialog.innerHTML = `<p class="eyebrow">INSTALL RESULT</p><h2 id="install-result-title" tabindex="-1"></h2><p id="install-result-description" class="modal-intro"></p><dl id="install-result-facts" class="detail-meta"></dl><p id="install-result-error" class="form-error" role="alert"></p><div class="modal-actions"><button class="secondary" id="install-result-done">Enter workspace</button><button class="primary" id="install-result-open">Open app</button></div>`;
document.body.append(dialog);
let currentApp = null;
dialog.addEventListener('cancel', event => {event.preventDefault(); closeDialog(dialog);});
dialog.addEventListener('close', () => {
  currentApp = null;
  const parent = document.getElementById(dialog.dataset.parentNav);
  if (parent) parent.setAttribute('aria-current', 'page');
});
document.getElementById('install-result-done').onclick = () => closeDialog(dialog);
document.getElementById('install-result-open').onclick = async () => {
  if (!currentApp || dialog.dataset.busy === 'true') return;
  const app = currentApp;
  const buttons = dialog.querySelectorAll('button');
  buttons.forEach(button => {button.disabled = true;});
  setDialogBusy(dialog, true);
  document.getElementById('install-result-error').textContent = '';
  try {
    await invoke('open_app', {id: app.id});
    setDialogBusy(dialog, false);
    await closeDialog(dialog);
  } catch (error) {
    document.getElementById('install-result-error').textContent = error?.message || 'Could not open the app. You can try again or check it in My Apps.';
  } finally {
    setDialogBusy(dialog, false);
    buttons.forEach(button => {button.disabled = false;});
  }
};

export async function showInstallResult(recipe, app, starter = false, linked = false) {
  markStarterStep(starter ? 'ready' : null, dialog);
  currentApp = app || null;
  dialog.querySelector('.eyebrow').textContent = linked ? 'CONNECTION SAVED' : 'INSTALL RESULT';
  document.getElementById('install-result-title').textContent = `${recipe.display_name} ${linked ? 'connected' : 'installed'}`;
  document.getElementById('install-result-done').className = `button ${linked ? 'primary' : 'secondary'}`;
  document.getElementById('install-result-open').hidden = linked;
  document.getElementById('install-result-description').textContent = app
    ? linked ? 'The address is saved in My Apps. Local Store does not manage or verify the external server.' : 'Saved to My Apps. Open it to finish any setup inside the app.'
    : linked ? 'The connection was saved, but My Apps could not be refreshed. Check the saved record before adding it again.' : 'Installation finished, but the saved app could not be refreshed. Check My Apps before opening or retrying setup.';
  const facts = document.getElementById('install-result-facts');
  facts.replaceChildren();
  for (const [name, value] of [['Saved state', app ? linked ? 'Linked address saved; availability not checked' : app.status : 'Not refreshed'], [linked ? 'Saved address' : 'Local address', app?.launch_url], ['Version', recipe.version], ['Data policy', linked ? 'Removing this connection keeps the external server and its data unchanged.' : 'Uninstall keeps data unless you explicitly choose to delete it.']]) {
    if (!value) continue;
    const term = document.createElement('dt'), detail = document.createElement('dd');
    term.textContent = name; detail.textContent = value;
    facts.append(term, detail);
  }
  document.getElementById('install-result-error').textContent = '';
  document.getElementById('install-result-open').disabled = !app;
  await showDialog(dialog);
  document.getElementById('install-result-title').focus();
}

export function showConnectionResult(name, app) { return showInstallResult({display_name:name},app,true,true); }
