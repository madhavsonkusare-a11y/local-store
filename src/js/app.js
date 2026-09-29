import { invoke, listenOperations, listenBrowserFailures, listenActivationFailures } from './api.js';
import { createOperations, operationLabel, diagnosticText, installStageLabel, canCancel, retryIsUnsafe } from './operations.js';
import { catalogControls, defaultFilters } from './catalog-controls.js';
import { discoveryCard, installedRow, emptyState, detail, recipeView, doctorView } from './render.js';
import { renderSetupFields, firstMissingAnswer, collectAnswers, markInvalidAnswer } from './setup-form.js';
import { showDialog, closeDialog, setDialogBusy, revealToast } from './motion.js';
import { createReadinessMonitor } from './readiness.js';
import { refreshEngineStatus } from './engine-settings.js';
import './recovery.js';

const $ = id => document.getElementById(id);
const state = { view: 'discover', filters: defaultFilters(), query: '', category: '', offset: 0, limit: 12, entries: [], apps: [], visibleApps: [], total: 0 };
let request = 0, recipeRequest = 0, searchTimer, toastTimer, pendingApp, activeRecipe, refreshError, activeCatalogId;
// Set when a failed install could not clean up after itself; blocks a retry
// that would run over containers or files still on disk.
let installNeedsReview = false;
const controls = catalogControls(state, render);
const message = error => error?.message || String(error);
const operations = createOperations(paintOperations);
// Readiness is answered per app after the list is on screen, so one slow
// address cannot hold up the others. Absent means not checked yet, or an
// address this build cannot probe; the row then keeps its container status.
const readiness = createReadinessMonitor({
  probe: app => invoke('app_readiness', {id: app.id}),
  changed: paintOperations,
});
const readinessLabel = { ready: 'ready', unreachable: 'not responding' };
function paintOperations() {
  readiness.update(state.visibleApps.map(app => ({...app, busy: Boolean(operations.get(app.id)?.pending)})),
    state.view === 'apps' && !document.hidden && !refreshError);
  const installing = activeRecipe && operations.get(activeRecipe.id);
  if (installing?.pending && installing.stage) {
    $('install-progress-title').textContent = installStageLabel(installing.stage);
  }
  // The control disappears once the commit stage is reported: past that point
  // the backend ignores a cancel, so offering one would be a false promise.
  const stop = $('install-stop');
  const offer = canCancel(installing);
  if (stop.hidden === offer) {
    // Never strand keyboard focus on a control that is going away. The
    // confirm button is disabled mid-install, so focus moves to the live
    // progress region: it keeps the reader in context and dismisses nothing.
    if (!offer && document.activeElement === stop) $('install-progress').focus();
    stop.hidden = !offer;
  }
  document.querySelectorAll('.installed-app').forEach(row => {
    const entry = operations.get(row.dataset.appId);
    const busy = Boolean(entry?.pending);
    row.classList.toggle('operation-pending', busy);
    const status = row.querySelector('.status');
    const settled = readiness.get(row.dataset.appId);
    const label = busy
      ? (entry.finishing ? 'Updating…' : operationLabel(entry.kind))
      : (readinessLabel[settled] || row.dataset.status);
    if (status.textContent !== label) status.textContent = label;
    status.classList.toggle('status-busy', busy);
    status.classList.toggle('status-unreachable', !busy && settled === 'unreachable');
    row.querySelector('.installed-actions').setAttribute('aria-busy', String(busy));
    row.querySelectorAll('.installed-actions button:not([data-logs])').forEach(button => {
      if (busy) button.setAttribute('aria-disabled', 'true');
      else button.removeAttribute('aria-disabled');
    });
    const app = state.apps.find(app => app.id === row.dataset.appId);
    const diagnostic = row.querySelector('.inline-error');
    const text = diagnosticText(entry?.error || (!busy && app?.status_error));
    if (diagnostic.textContent !== text) diagnostic.textContent = text;
  });
}
async function invokeOperation(kind, appId, command, args, settled = async () => {}) {
  const token = operations.begin(appId, kind);
  if (!token) throw {code: 'operation_busy', message: 'This app already has an operation in progress.'};
  let failure = null;
  try { const result = await invoke(command, args); await settled(); return result; }
  catch (error) { failure = error; throw error; }
  finally { operations.finish(appId, token, failure); }
}
function toast(text) {
  clearTimeout(toastTimer);
  $('toast').textContent = text;
  $('toast').hidden = false;
  revealToast($('toast'));
  toastTimer = setTimeout(() => { $('toast').hidden = true; }, 6000);
}
function loadState(loading) { $('content').setAttribute('aria-busy', String(loading)); }
async function refreshApps() {
  try {
    state.apps = await invoke('list_apps');
    refreshError = null;
    $('app-count').textContent = state.apps.length;
  } catch (error) { refreshError = error; $('app-count').textContent = '–'; }
}
async function render() {
  const token = ++request;
  $('pagination').hidden = true;
  if (state.view === 'apps') {
    loadState(false);
    if (refreshError) { readiness.update([], false); showError(refreshError); return; }
    const query = state.query.toLowerCase();
    state.visibleApps = state.apps.filter(app => `${app.display_name} ${app.launch_url}`.toLowerCase().includes(query));
    $('results-count').textContent = `${state.visibleApps.length} ${state.visibleApps.length === 1 ? 'app' : 'apps'}`;
    const focused = document.activeElement;
    const focusedId = focused?.closest('.installed-app')?.dataset.appId;
    const focusedAction = focusedId && Object.keys(focused.dataset)[0];
    const focusedIndex = [...document.querySelectorAll('.installed-app')].findIndex(row => row.dataset.appId === focusedId);
    $('content').innerHTML = state.visibleApps.length
      ? `<div class="installed-list">${state.visibleApps.map(installedRow).join('')}</div>`
      : emptyState(state.query ? 'No matching apps' : 'Your apps belong here.', state.query ? 'Try another name or address.' : 'Install a reviewed recipe or connect an app you already run.', state.query ? 'clear' : 'discover', state.query ? 'Clear search' : 'Discover apps');
    paintOperations();
    if (focusedId) {
      const rows = [...document.querySelectorAll('.installed-app')];
      const row = rows.find(row => row.dataset.appId === focusedId) || rows[Math.min(focusedIndex, rows.length - 1)];
      (row?.querySelector(`[data-${focusedAction}]`) || row?.querySelector('.primary') ||
        $('content').querySelector('.empty-state button'))?.focus({preventScroll: true});
    }
    return;
  }
  loadState(true);
  // Keep the previous results in place during search; avoid a loading flash on every key.
  if (!$('content').children.length) $('content').innerHTML = '<div class="loading" role="status">Finding your next app…</div>';
  try {
    const page = await invoke('search_catalog', { query: state.query, category: state.category, offset: state.offset, limit: state.limit, filters: state.filters });
    if (token !== request) return;
    state.entries = page.entries.map(app => ({...app, snapshot_date:page.snapshot_date}));
    state.total = page.total;
    controls.update(page);
    $('results-count').textContent = `${page.total.toLocaleString()} ${page.total === 1 ? 'project' : 'projects'}`;
    $('catalog-note').textContent = `${page.catalog_total.toLocaleString()} projects to discover · Works offline · Install previews are marked`;
    $('content').innerHTML = page.entries.length
      ? `<div class="app-grid">${page.entries.map(discoveryCard).join('')}</div>`
      : emptyState('Nothing here just yet.', 'Try a different search or category. You can also connect an app that isn’t in this collection.', 'clear', 'Clear filters', 'search');
    $('pagination').hidden = page.total <= state.limit;
    $('page-label').textContent = `${state.offset + 1}–${Math.min(state.offset + state.limit, page.total)} of ${page.total.toLocaleString()} projects`;
    $('previous').disabled = state.offset === 0;
    $('next').disabled = state.offset + state.limit >= page.total;
  } catch (error) { if (token === request) showError(error); }
  finally { if (token === request) loadState(false); }
}
function showError(error) { $('content').innerHTML = emptyState('We couldn’t load your workspace.', message(error), 'retry', 'Try again', 'circle-alert'); }
function navigate(view) {
  clearTimeout(searchTimer);
  state.view = view; state.query = ''; state.category = ''; state.offset = 0;
  readiness.update([], false);
  $('search').value = ''; controls.reset();
  const discover = view === 'discover';
  for (const [id, active] of [['nav-discover', discover], ['nav-apps', !discover]]) {
    $(id).classList.toggle('selected', active);
    if (active) $(id).setAttribute('aria-current', 'page'); else $(id).removeAttribute('aria-current');
  }
  $('breadcrumb').textContent = discover ? 'Discover' : 'My Apps';
  $('eyebrow').textContent = discover ? 'THE SELF-HOSTED COLLECTION' : 'YOUR PERSONAL WORKSPACE';
  $('page-title').textContent = discover ? 'Good software. Your space.' : 'Right where you left them.';
  $('intro').textContent = discover ? 'Discover independent apps. Bring your favorites closer to home.' : 'Your apps, their own windows. All within reach.';
  $('results-title').textContent = discover ? 'Explore the collection' : 'Your apps';
  $('search').placeholder = discover ? 'Search apps, ideas, and tools…' : 'Search your apps…';
  for (const id of ['discovery-note', 'filter-open', 'featured-shelf', 'collections', 'catalog-controls', 'active-filters']) $(id).hidden = !discover;
  $('catalog-note').textContent = discover ? 'Independent software. A little closer to home.' : 'Your app registry is saved on this computer.';
  $('content').replaceChildren();
  window.scrollTo({ top: 0, behavior: 'instant' });
  render();
}
function openConnect(name = '', catalogId = null) {
  activeCatalogId = catalogId;
  $('connect-name').removeAttribute('aria-invalid'); $('connect-url').removeAttribute('aria-invalid');
  $('connect-check').textContent = '';
  $('connect-form').reset();
  $('connect-name').value = name;
  $('connect-error').textContent = '';
  showDialog($('connect-dialog'));
  (name ? $('connect-url') : $('connect-name')).focus();
}
// The port field stays hidden until asked for: the reviewed dialog should look
// the same for the ordinary case, where the pinned port is simply used.
function wirePortChoice(recipe) {
  const reveal = $('change-port'), field = $('port-field'), help = $('port-help'), input = $('recipe-port');
  if (!reveal) return;
  reveal.onclick = () => {
    reveal.hidden = true; field.hidden = false; help.hidden = false;
    input.focus(); input.select();
  };
  input.oninput = () => {
    // Keep the stated address honest while the number is being edited.
    const chosen = chosenPort();
    $('recipe-address').textContent = chosen
      ? recipe.launch_url.replace(`:${recipe.host_port}`, `:${chosen}`)
      : recipe.launch_url;
    $('install-error').textContent = '';
  };
}
// The chosen port, or null when the field is untouched or not a usable port.
function chosenPort() {
  const input = $('recipe-port');
  if (!input || $('port-field').hidden) return null;
  const value = Number(input.value);
  return Number.isInteger(value) && value >= 1024 && value <= 65535 ? value : null;
}
async function reviewInstall(recipeId) {
  const token = ++recipeRequest;
  activeRecipe = null;
  installNeedsReview = false;
  $('install-title').textContent = 'Review installation';
  $('install-content').innerHTML = '<div class="loading" role="status">Checking Docker and recipe…</div>';
  $('install-error').textContent = '';
  $('install-confirm').textContent = 'Checking system…';
  $('install-confirm').disabled = true;
  showDialog($('install-dialog'));
  try {
    const [recipe, report] = await Promise.all([invoke('recipe_details', { id: recipeId }), invoke('doctor')]);
    if (token !== recipeRequest || !$('install-dialog').open) return;
    activeRecipe = recipe;
    $('install-title').textContent = `Review ${recipe.display_name} installation`;
    $('install-confirm').textContent = `Install ${recipe.display_name}`;
    $('install-confirm').disabled = !report.ready;
    $('install-content').innerHTML = recipeView(recipe, report);
    wirePortChoice(recipe);
    // Rebuilt from the projection every time the review opens, so nothing a
    // previous review was given survives into this one.
    renderSetupFields($('install-content'), recipe.setup_review);
    if (!report.ready) $('install-error').textContent = 'Start Docker Desktop and make sure Docker Compose is available, then reopen this review.';
  } catch (error) {
    if (token !== recipeRequest || !$('install-dialog').open) return;
    $('install-content').innerHTML = '';
    $('install-error').textContent = diagnosticText(error);
    $('install-confirm').textContent = 'Install unavailable';
  }
}
function showDetail(app) {
  $('detail-content').innerHTML = detail(app);
  if (app.capability === 'preview_install') {
    const connect = document.createElement('button');
    connect.id = 'detail-connect';
    connect.className = 'text-button detail-connect';
    connect.textContent = 'Already running it? Connect an instance';
    $('detail-content').append(connect);
  }
  showDialog($('detail-dialog'));
  $('detail-primary').onclick = async () => {
    await closeDialog($('detail-dialog'));
    if (app.capability === 'preview_install') reviewInstall(app.recipe_id);
    else if (app.capability === 'discover') { try { await invoke('open_project', { url: app.website_url || app.source_url }); } catch (error) { toast(message(error)); } }
    else openConnect(app.name, app.id);
  };
  $('detail-connect')?.addEventListener('click', async () => { await closeDialog($('detail-dialog')); openConnect(app.name, app.id); });
  $('open-source').onclick = async () => {
    try { await invoke('open_project', { url: app.source_url }); }
    catch (error) { $('detail-error').textContent = message(error); }
  };
}
async function runAppAction(command, app, success) {
  try {
    await invokeOperation(command === 'start_app' ? 'start' : 'stop', app.id, command, {id: app.id}, async () => { await refreshApps(); await render(); });
    toast(success);
  } catch { /* The per-app diagnostic survives navigation and re-rendering. */ }
}
$('nav-discover').onclick = () => navigate('discover');
$('nav-apps').onclick = async () => { await refreshApps(); navigate('apps'); };
document.querySelector('.brand').onclick = event => { event.preventDefault(); navigate('discover'); };
$('connect-top').onclick = $('connect-note').onclick = () => openConnect();
$('about').onclick = () => showDialog($('about-dialog'));
$('settings').onclick = () => { showDialog($('settings-dialog')); void refreshEngineStatus(); };
$('run-doctor').onclick = async () => {
  const button = $('run-doctor'); button.disabled = true; button.textContent = 'Checking…';
  $('doctor-error').textContent = ''; $('doctor-output').innerHTML = '';
  try { $('doctor-output').innerHTML = doctorView(await invoke('doctor')); }
  catch (error) { $('doctor-error').textContent = message(error); }
  finally { button.disabled = false; button.textContent = 'Run again'; }
};
$('search').addEventListener('input', () => {
  clearTimeout(searchTimer); ++request; state.query = $('search').value; state.offset = 0;
  searchTimer = setTimeout(render, 180);
});
$('previous').onclick = () => { state.offset = Math.max(0, state.offset - state.limit); render(); };
$('next').onclick = () => { state.offset += state.limit; render(); };
document.addEventListener('keydown', event => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k' && !document.querySelector('dialog[open]')) { event.preventDefault(); $('search').focus(); }
});
document.addEventListener('error', event => { if (event.target.matches?.('.app-avatar img')) event.target.remove(); }, true);
document.addEventListener('click', async event => {
  const button = event.target.closest('button');
  if (!button || button.getAttribute('aria-disabled') === 'true') return;
  if (button.dataset.projectUrl) { try { await invoke('open_project', { url: button.dataset.projectUrl }); } catch (error) { $('detail-error').textContent = message(error); } return; }
  if (button.dataset.close) { await closeDialog($(button.dataset.close)); return; }
  if (button.dataset.featured) { reviewInstall(button.dataset.featured); return; }
  if (button.dataset.detail !== undefined) { showDetail(state.entries[Number(button.dataset.detail)]); return; }
  if (button.dataset.action === 'connect') openConnect();
  if (button.dataset.action === 'discover') navigate('discover');
  if (button.dataset.action === 'clear') {
    clearTimeout(searchTimer);
    $('search').value = ''; controls.reset(); state.query = ''; state.category = ''; state.offset = 0; render();
  }
  if (button.dataset.action === 'retry') { await refreshApps(); render(); }
  const keyed = ['open', 'shortcut', 'start', 'stop', 'logs', 'remove', 'uninstall'].find(key => button.dataset[key] !== undefined);
  if (!keyed) return;
  const app = state.visibleApps[Number(button.dataset[keyed])];
  if (!app || (keyed !== 'logs' && operations.get(app.id)?.pending)) return;
  if (keyed === 'remove') {
    pendingApp = app; $('remove-description').textContent = `${app.display_name} will be removed from My Apps.`;
    $('remove-error').textContent = ''; showDialog($('remove-dialog')); return;
  }
  if (keyed === 'uninstall') {
    pendingApp = app; $('delete-data').checked = false; $('uninstall-confirm').textContent = 'Uninstall, keep data';
    $('uninstall-description').textContent = `${app.display_name} and its container will be removed. Its data is preserved by default.`;
    $('uninstall-error').textContent = ''; showDialog($('uninstall-dialog')); return;
  }
  if (keyed === 'logs') {
    $('logs-title').textContent = `${app.display_name} logs`; $('logs-content').textContent = 'Loading…'; $('logs-error').textContent = '';
    showDialog($('logs-dialog'));
    try { $('logs-content').textContent = await invoke('app_logs', { id: app.id }) || 'No recent output.'; }
    catch (error) { $('logs-content').textContent = ''; $('logs-error').textContent = message(error); }
    return;
  }
  if (keyed === 'start' || keyed === 'stop') { await runAppAction(keyed === 'start' ? 'start_app' : 'stop_app', app, `${app.display_name} ${keyed === 'start' ? 'started' : 'stopped'}.`); return; }
  try {
    await invokeOperation(keyed, app.id, keyed === 'open' ? 'open_app' : 'create_shortcut', {id: app.id});
    toast(keyed === 'open' ? `${app.display_name} opened.` : `Shortcut created for ${app.display_name}.`);
  } catch { /* Rendered by stable app ID, even if the user navigates away. */ }

});
$('connect-name').addEventListener('input', () => { $('connect-name').removeAttribute('aria-invalid'); $('connect-error').textContent = ''; });
$('connect-url').addEventListener('input', () => { $('connect-url').removeAttribute('aria-invalid'); $('connect-error').textContent = ''; $('connect-check').textContent = ''; });
// Advisory only. An app the user simply has not started yet is a perfectly good
// connection to save, so a failed check never blocks the form.
$('connect-check-button').onclick = async () => {
  const url = $('connect-url').value.trim();
  if (!url) { $('connect-check').textContent = 'Enter an address to check.'; return; }
  $('connect-check-button').disabled = true;
  $('connect-check').textContent = 'Checking the address…';
  try {
    const result = await invoke('check_address', { url });
    $('connect-check').textContent = {
      ready: 'That address answered. It is ready to add.',
      unreachable: 'No answer at that address right now. You can still add it — start the app later and open it from My Apps.',
      unknown: 'This address cannot be checked from here, so add it and open it to confirm.',
    }[result] || 'This address cannot be checked from here, so add it and open it to confirm.';
  } catch (error) {
    $('connect-check').textContent = '';
    $('connect-error').textContent = message(error);
    $('connect-url').setAttribute('aria-invalid', 'true');
  } finally { $('connect-check-button').disabled = false; }
};
$('connect-form').onsubmit = async event => {
  event.preventDefault();
  const name = $('connect-name').value.trim(), rawUrl = $('connect-url').value.trim();
  try {
    const url = new URL(rawUrl);
    if (!/^https?:\/\//i.test(rawUrl) || !['http:', 'https:'].includes(url.protocol) || !url.hostname || url.username || url.password || /\s/.test(rawUrl)) throw new Error('Enter an http:// or https:// address without embedded credentials.');
    if (!name) throw new Error('Enter a name for this connection.');
    $('connect-submit').disabled = true; $('connect-submit').textContent = 'Saving…'; $('connect-error').textContent = '';
    setDialogBusy($('connect-dialog'), true);
    await invoke('add_app', { name, url: rawUrl, ...(activeCatalogId ? { catalogId: activeCatalogId } : {}) });
    setDialogBusy($('connect-dialog'), false);
    await closeDialog($('connect-dialog'));
    await refreshApps(); navigate('apps'); toast(`${name} added to My Apps.`);
  } catch (error) {
    const text = message(error);
    $('connect-error').textContent = text;
    if (/http|address|url/i.test(text)) $('connect-url').setAttribute('aria-invalid', 'true');
    else if (/name/i.test(text)) $('connect-name').setAttribute('aria-invalid', 'true');
  }
  finally { setDialogBusy($('connect-dialog'), false); $('connect-submit').disabled = false; $('connect-submit').textContent = 'Add to My Apps →'; }
};
$('install-stop').onclick = async () => {
  const entry = activeRecipe && operations.get(activeRecipe.id);
  if (!canCancel(entry)) return;
  const operationId = entry.cancelId;
  // Stop offering immediately; IPC still settles the install either way.
  operations.cancelRequested(activeRecipe.id);
  $('install-progress-title').textContent = 'Stopping setup…';
  try { await invoke('cancel_app_setup', { id: activeRecipe.id, operationId }); }
  catch { /* IPC completion of the install remains authoritative. */ }
};
$('install-confirm').onclick = async () => {
  if (!activeRecipe || operations.get(activeRecipe.id)?.pending) return;
  const recipe = activeRecipe;
  if (!$('port-field')?.hidden && chosenPort() === null) {
    $('install-error').textContent = 'Choose a port between 1024 and 65535.';
    $('recipe-port').setAttribute('aria-invalid', 'true');
    $('recipe-port').focus();
    return;
  }
  const missing = firstMissingAnswer();
  if (missing) {
    $('install-error').textContent = 'Fill in the required setup fields.';
    missing.setAttribute('aria-invalid', 'true');
    missing.focus();
    return;
  }
  $('recipe-port')?.removeAttribute('aria-invalid');
  $('install-confirm').disabled = true; $('install-confirm').textContent = 'Installing…'; $('install-error').textContent = '';
  setDialogBusy($('install-dialog'), true);
  $('install-progress').hidden = false;
  $('install-progress-title').textContent = `Installing ${recipe.display_name}`;
  // Keep the live status outside an aria-busy subtree; dismissal still uses data-busy.
  $('install-dialog').removeAttribute('aria-busy');
  $('install-progress').scrollIntoView({block: 'nearest', behavior: 'instant'});
  try {
    // Omitted entirely when untouched, so the ordinary install sends exactly
    // what it always sent and uses the recipe's pinned port.
    const chosen = chosenPort();
    // Read at the moment of sending and never kept: answers exist in the form
    // and in this call, and nowhere else.
    const answers = collectAnswers();
    await invokeOperation('install', recipe.id, 'install_app', {
      recipeId: recipe.id,
      ...(chosen === null ? {} : { hostPort: chosen }),
      ...(Object.keys(answers).length ? { answers } : {}),
    });
    setDialogBusy($('install-dialog'), false);
    await closeDialog($('install-dialog'));
    await refreshApps(); navigate('apps'); toast(`${recipe.display_name} installed and ready.`);
  } catch (error) {
    const text = diagnosticText(error);
    $('install-error').textContent = text;
    markInvalidAnswer(text);
    installNeedsReview = retryIsUnsafe(error);
  }
  finally {
    $('install-progress').hidden = true; $('install-stop').hidden = true;
    setDialogBusy($('install-dialog'), false);
    if (installNeedsReview) {
      // Deliberately not re-enabled: the user has to look at what was left
      // behind before this can be tried again.
      $('install-confirm').disabled = true;
      $('install-confirm').textContent = 'Review needed before retrying';
    } else {
      $('install-confirm').disabled = false;
      $('install-confirm').textContent = `Install ${recipe.display_name}`;
    }
  }
};
$('remove-confirm').onclick = async () => {
  const app = pendingApp;
  $('remove-confirm').disabled = true; setDialogBusy($('remove-dialog'), true);
  try {
    await invokeOperation('remove', app.id, 'remove_app_cmd', { id: app.id });
    setDialogBusy($('remove-dialog'), false); await closeDialog($('remove-dialog'));
    await refreshApps(); render(); toast('Connection removed. App data is unchanged.');
  } catch (error) { $('remove-error').textContent = message(error); }
  finally { $('remove-confirm').disabled = false; setDialogBusy($('remove-dialog'), false); }
};
$('delete-data').onchange = () => { $('uninstall-confirm').textContent = $('delete-data').checked ? 'Uninstall and delete data' : 'Uninstall, keep data'; };
$('uninstall-confirm').onclick = async () => {
  const app = pendingApp, deleteData = $('delete-data').checked;
  if (deleteData && !window.confirm('Permanently delete this app’s managed data? This cannot be undone.')) return;
  $('delete-data').disabled = true;
  $('uninstall-confirm').disabled = true; setDialogBusy($('uninstall-dialog'), true);
  try {
    await invokeOperation('uninstall', app.id, 'uninstall_app', { id: app.id, deleteData });
    setDialogBusy($('uninstall-dialog'), false); await closeDialog($('uninstall-dialog'));
    await refreshApps(); render(); toast(deleteData ? 'App and managed data deleted.' : 'App uninstalled. Managed data was preserved.');
  } catch (error) { $('uninstall-error').textContent = diagnosticText(error); }
  finally { $('delete-data').disabled = false; $('uninstall-confirm').disabled = false; setDialogBusy($('uninstall-dialog'), false); }
};
// Subscribe before actions are enabled. Missing event support must not block IPC.
let unlisten = () => {};
try { unlisten = await listenOperations(event => operations.receive(event)); } catch { /* IPC is authoritative. */ }
try { await listenBrowserFailures(error => toast(diagnosticText(error))); } catch { /* A missing channel must not break the launcher. */ }
let unlistenActivation = () => {};
try { unlistenActivation = await listenActivationFailures(error => toast(diagnosticText(error))); } catch { /* Keep ordinary launcher actions available. */ }
document.addEventListener('visibilitychange', paintOperations);
window.addEventListener('pagehide', () => { readiness.dispose(); unlisten(); unlistenActivation(); }, {once: true});
await refreshApps();
render();
