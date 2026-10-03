import {invoke} from './api.js';
import {showDialog, closeDialog, setDialogBusy} from './motion.js';
import {doctorView} from './render.js';
const dialog = document.getElementById('first-run-dialog');
const error = document.getElementById('first-run-error');
const steps = ['welcome_viewed','engine_explanation_viewed','first_app_explanation_viewed','agent_access_explanation_viewed'];
let reviewStarter, connectStarter, chosen = 'memos', selectedPort = null, loadGeneration = 0;
const choose = document.createElement('section');
choose.id = 'starter-choice'; choose.hidden = true;
choose.innerHTML = `<h3 id="starter-choice-title" tabindex="-1">Choose your first app</h3><p class="modal-intro">Review an installation next. Choosing an app does not install it or grant agent access.</p><div id="starter-engine-checks"></div><fieldset id="starter-cards"><legend>Reviewed starter apps</legend></fieldset><label class="field">Published port<input id="starter-port" type="number" min="1024" max="65535" inputmode="numeric"></label><p id="starter-preflight" role="status"></p><div class="modal-actions"><button class="secondary" id="starter-back">Back to welcome</button><button class="primary" id="starter-review" disabled>Review installation</button></div>`;
dialog.insertBefore(choose, error);
const start = document.createElement('button'); start.className = 'primary'; start.id = 'first-run-start'; start.textContent = 'Get started';
document.getElementById('first-run-done').closest('.modal-actions').append(start);
document.getElementById('first-run-done').className = 'secondary';
const connect = document.createElement('button'); connect.className = 'secondary'; connect.textContent = 'Connect an existing app'; connect.id = 'first-run-connect';
start.before(connect);
connect.onclick = async () => { await closeDialog(dialog); connectStarter?.(); };
const welcomeNodes = [...dialog.children].filter(node => node !== choose && node !== error && !node.classList.contains('modal-top') && node.id !== 'first-run-title');
const layout = document.createElement('div'); layout.className = 'first-run-layout';
const rail = document.createElement('ol'); rail.className = 'first-run-rail'; rail.setAttribute('aria-label', 'First app setup');
for (const label of ['Welcome', 'Choose', 'Install', 'Ready']) { const step = document.createElement('li'); step.textContent = label; rail.append(step); }
const pane = document.createElement('div'); pane.className = 'first-run-pane';
for (const node of [...dialog.children]) if (!node.classList.contains('modal-top')) pane.append(node);
layout.append(rail, pane); dialog.append(layout);
const welcomeChecks = document.createElement('section'); welcomeChecks.id = 'welcome-engine-checks'; welcomeChecks.setAttribute('aria-label','Local engine preflight');
pane.querySelector('.onboarding-steps').before(welcomeChecks);
let welcomeCheck = 0;
async function checkWelcomeEngine() {
  const generation = ++welcomeCheck;
  welcomeChecks.innerHTML = '<p role="status">Checking the local engine…</p>';
  try {
    const report = await invoke('doctor');
    if (generation !== welcomeCheck || !dialog.open) return;
    welcomeChecks.innerHTML = Array.isArray(report?.checks) ? doctorView(report) : '';
    const status = document.createElement('p'); status.setAttribute('role','status');
    status.textContent = report?.ready ? 'Local engine ready. You can review your first app.' : 'The local engine needs setup. Explore apps now, or open Settings to review the setup choices.';
    welcomeChecks.append(status);
  } catch { if (generation === welcomeCheck && dialog.open) welcomeChecks.innerHTML = '<p role="status">Engine status could not be checked. You can explore apps or check again in Settings.</p>'; }
}
export function markStarterStep(step, target) {
  target.querySelector('.first-run-task-rail')?.remove();
  if (!step) return;
  const progress = rail.cloneNode(true); progress.className = 'first-run-task-rail';
  const current = step === 'ready' ? 3 : 2;
  [...progress.children].forEach((item,index) => { item.removeAttribute('aria-current'); if (index === current) item.setAttribute('aria-current','step'); });
  const heading = target.querySelector('h2'); heading?.before(progress);
}
const switchChoice = show => {
  welcomeNodes.forEach(node => {node.hidden = show;}); welcomeChecks.hidden = show; choose.hidden = !show;
  document.getElementById(show ? 'starter-choice-title' : 'first-run-title').focus();
  [...rail.children].forEach((step, index) => {if (index === (show ? 1 : 0)) step.setAttribute('aria-current', 'step'); else step.removeAttribute('aria-current');});
};
document.getElementById('starter-back').onclick = () => switchChoice(false);
start.onclick = async () => {
  switchChoice(true);
  const generation = ++loadGeneration;
  const cards = document.getElementById('starter-cards'); cards.replaceChildren();
  const review = document.getElementById('starter-review'); review.disabled = true;
  const status = document.getElementById('starter-preflight'); status.textContent = 'Checking the local engine…';
  document.getElementById('starter-engine-checks').replaceChildren();
  const doctor = invoke('doctor').then(report => {
    if (generation !== loadGeneration) return;
    if (Array.isArray(report.checks)) document.getElementById('starter-engine-checks').innerHTML = doctorView(report);
    status.textContent = report.ready ? 'Local engine ready. Review the app before installation.' : 'Set up the local engine in Settings before installing. You can still review or explore apps.';
  }).catch(() => {if (generation === loadGeneration) status.textContent = 'Engine status could not be checked. The installation review will check again.';});
  const results = await Promise.allSettled(['memos','n8n','uptime-kuma'].map(id => invoke('recipe_details', {id})));
  if (generation !== loadGeneration || !dialog.open) return;
  const recipes = results.filter(result => result.status === 'fulfilled' && result.value?.id).map(result => result.value);
  if (!recipes.some(recipe => recipe.id === chosen)) {chosen = recipes[0]?.id; selectedPort = null;}
  for (const recipe of recipes) {
    const label = document.createElement('label'); label.className = 'starter-card';
    const radio = document.createElement('input'); radio.type = 'radio'; radio.name = 'starter-app'; radio.value = recipe.id; radio.checked = chosen === recipe.id;
    const copy = document.createElement('span'), name = document.createElement('strong'), description = document.createElement('small'), facts = document.createElement('small');
    name.textContent = recipe.display_name; description.textContent = recipe.description; facts.textContent = `${recipe.version} · ${recipe.license}`;
    const icon = document.createElement('img'); icon.className = 'starter-icon'; icon.alt = ''; icon.src = `assets/apps/${recipe.id}.svg`;
    copy.append(name, description, facts); label.append(radio, icon, copy); cards.append(label);
    radio.onchange = () => {chosen = recipe.id; selectedPort = recipe.host_port; document.getElementById('starter-port').value = selectedPort; error.textContent = '';};
  }
  const selected = recipes.find(recipe => recipe.id === chosen);
  document.getElementById('starter-port').value = selectedPort ?? selected?.host_port ?? '';
  review.disabled = !selected || !reviewStarter;
  if (!selected) error.textContent = 'Starter recipes are unavailable. You can explore the catalog or try again.';
  await doctor;
};
document.getElementById('starter-review').onclick = async () => {
  const input = document.getElementById('starter-port');
  const port = Number(input.value);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) {error.textContent = 'Choose a port between 1024 and 65535.'; input.focus(); return;}
  selectedPort = port;
  await closeDialog(dialog);
  await reviewStarter(chosen, port);
};
export function initializeFirstRun({reviewInstall, connectExisting}) {reviewStarter = reviewInstall; connectStarter = connectExisting;}
export async function returnToStarterChoice() {
  switchChoice(true);
  await showDialog(dialog);
  document.getElementById('starter-port').focus();
}
export async function finishStarterIntroduction() {
  try { for (const step of steps) await invoke('mark_onboarding_viewed', {step}); }
  catch { /* A completed install must never become a failure because introduction progress could not be saved. */ }
}
async function acknowledge(openSettings) {
  setDialogBusy(dialog, true);
  const buttons = dialog.querySelectorAll('.modal-actions button');
  buttons.forEach(button => {button.disabled = true;});
  error.textContent = '';
  try {
    // Viewing explanations is presentation progress, never setup or agent consent.
    for (const step of steps) await invoke('mark_onboarding_viewed', {step});
    setDialogBusy(dialog, false);
    await closeDialog(dialog);
    if (openSettings) document.getElementById('settings').click();
    else document.getElementById('nav-discover').focus();
  } catch (failure) { error.textContent = failure.message || 'Could not save introduction progress. You can try again.'; }
  finally { setDialogBusy(dialog, false); buttons.forEach(button => {button.disabled = false;}); }
}
document.getElementById('first-run-done').onclick = () => acknowledge(false);
document.getElementById('first-run-settings').onclick = () => acknowledge(true);
export async function showFirstRun(force = false) {
  try {
    const progress = await invoke('onboarding_progress');
    if (force || (progress && Array.isArray(progress.viewed) && !steps.every(step => progress.viewed.includes(step)))) {
      switchChoice(false);
      await showDialog(dialog);
      document.getElementById('first-run-title').focus();
      void checkWelcomeEngine();
    }
  } catch { /* Intro progress cannot block the person's existing apps. */ }
}
dialog.addEventListener('close', () => { welcomeCheck++; });
const replay = document.createElement('button'); replay.className = 'secondary'; replay.id = 'replay-introduction'; replay.textContent = 'Run introduction';
replay.onclick = async () => { await closeDialog(document.getElementById('settings-dialog')); await showFirstRun(true); };
const introduction = document.createElement('section'); introduction.className = 'settings-row';
const label = document.createElement('div'); label.innerHTML = '<h3>First app setup</h3><p>Revisit the introduction or choose a starter app.</p>';
introduction.append(label, replay); document.getElementById('settings-dialog').append(introduction);
