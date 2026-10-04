import {publishEngineStatus, refreshRailEngine} from './rail-engine.js';
import {loadingComposition} from './loading-composition.js';
import './v2-primitives.js';
import { invoke, listenOperations, listenBrowserFailures, listenActivationFailures } from './api.js';
import { createOperations, operationLabel, diagnosticText, installStageLabel, canCancel, retryIsUnsafe } from './operations.js';
import { catalogControls, defaultFilters } from './catalog-controls.js';
import { discoveryCard, installedRow, emptyState, detail, recipeView, doctorView } from './render.js';
import { renderSetupFields, firstMissingAnswer, collectAnswers, markInvalidAnswer } from './setup-form.js';
import { showDialog, closeDialog, setDialogBusy, revealToast } from './motion.js';
import { createReadinessMonitor } from './readiness.js';
import { refreshEngineStatus } from './engine-settings.js';
import { renderOverview } from './overview.js';
import { myAppsDetail } from './my-apps.js';
import './recovery.js';
import {refreshAgents} from './agent-controls.js';
import './agent-content-controls.js';
import './agent-data-controls.js';
import {initializeGithubReview} from './github-review.js';
import {readinessView} from './launch-readiness.js';
import {showFirstRun, initializeFirstRun, returnToStarterChoice, finishStarterIntroduction, markStarterStep} from './first-run.js';
import {showInstallResult, showConnectionResult} from './install-result.js';
import {resetInstallTask,renderInstallStage,renderInstallFailure} from './install-task-view.js';
import {refreshLaunchCollection} from './launch-collection.js';
import {beginActivity, finishActivity, receiveActivity, renderActivity} from './activity.js';

const $ = id => document.getElementById(id);
const state = { view: 'discover', filters: defaultFilters(), query: '', category: '', offset: 0, limit: 24, entries: [], apps: [], visibleApps: [], total: 0 };
let request = 0, recipeRequest = 0, searchTimer, toastTimer, pendingApp, activeRecipe, refreshError, activeCatalogId;
let selectedAppId = null, detailTab = 'overview', detailLog = {state:'idle'};
// Set when a failed install could not clean up after itself; blocks a retry
// that would run over containers or files still on disk.
let installNeedsReview = false;
let starterReview = false, starterConnection = false;
let beforeSettings = 'discover';
const settingsScreen = $('settings-dialog');
// Keep the existing Settings controllers' visibility/close contract while
// presenting an ordinary semantic workspace section, with no modal behavior.
Object.defineProperty(settingsScreen,'open',{get:() => !settingsScreen.hidden});
settingsScreen.show = () => {settingsScreen.hidden = false;};
settingsScreen.close = () => {if (settingsScreen.hidden) return; settingsScreen.hidden = true; settingsScreen.dispatchEvent(new Event('close'));};
document.querySelector('.workspace').append(settingsScreen);
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
function selectedDetail(app, tab, log) { return myAppsDetail(app, tab, log, state.visibleApps.findIndex(item => item.id === app?.id)); }
function paintOperations() {
  readiness.update(state.visibleApps.map(app => ({...app, busy: Boolean(operations.get(app.id)?.pending)})),
    state.view === 'apps' && !document.hidden && !refreshError);
  const installing = activeRecipe && operations.get(activeRecipe.id);
  if (installing?.pending && installing.stage) {
    $('install-progress-title').textContent = installStageLabel(installing.stage);
    renderInstallStage(installing.stage);
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
    status.classList.toggle('status-ready', !busy && settled === 'ready');

    row.querySelectorAll('.installed-actions button:not([data-logs])').forEach(button => {
      if (busy) button.setAttribute('aria-disabled', 'true');
      else button.removeAttribute('aria-disabled');
    });
    const app = state.apps.find(app => app.id === row.dataset.appId);
    const diagnostic = row.querySelector('.inline-error');
    const text = diagnosticText(entry?.error || (!busy && app?.status_error));
    if (diagnostic.textContent !== text) diagnostic.textContent = text;
  });
  const detailActions = document.querySelector('#my-apps-detail .installed-actions');
  if (detailActions) {
    const busy = Boolean(operations.get(selectedAppId)?.pending);
    detailActions.setAttribute('aria-busy', String(busy));
    detailActions.querySelectorAll('button:not([data-logs])').forEach(button => {
      if (busy) button.setAttribute('aria-disabled', 'true'); else button.removeAttribute('aria-disabled');
    });
  }
  const selectedStatus = document.querySelector('.installed-app.selected .status');
  const detailStatus = $('my-apps-detail-status');
  if (selectedStatus && detailStatus) detailStatus.textContent = selectedStatus.textContent;
}
async function invokeOperation(kind, appId, command, args, settled = async () => {}) {
  const token = operations.begin(appId, kind);
  if (!token) throw {code: 'operation_busy', message: 'This app already has an operation in progress.'};
  const activityId = beginActivity(kind, appId, state.apps.find(app => app.id === appId)?.display_name || (activeRecipe?.id === appId ? activeRecipe.display_name : appId));
  let failure = null;
  try { const result = await invoke(command, args); await settled(); return result; }
  catch (error) { failure = error; throw error; }
  finally { operations.finish(appId, token, failure); finishActivity(activityId, failure); }
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
async function render(append = false) {
  const token = ++request;
  if (state.view === 'overview' || state.view === 'activity') return;
  if (!append) $('pagination').hidden = true;
  if (state.view === 'apps') {
    loadState(false);
    if (refreshError && !state.apps.length) { readiness.update([], false); showError(refreshError); return; }
    const query = state.query.toLowerCase();
    state.visibleApps = state.apps.filter(app => `${app.display_name} ${app.launch_url}`.toLowerCase().includes(query));
    if (!state.visibleApps.some(app => app.id === selectedAppId)) {
      selectedAppId = state.visibleApps[0]?.id || null;
      detailTab = 'overview'; detailLog = {state:'idle'};
    }
    $('results-count').textContent = `${state.visibleApps.length} ${state.visibleApps.length === 1 ? 'app' : 'apps'}`;
    const focused = document.activeElement;
    const focusedId = focused?.closest('[data-app-id]')?.dataset.appId;
    const focusedAction = focusedId && Object.keys(focused.dataset)[0];
    const focusedInDetail = Boolean(focused?.closest('#my-apps-detail'));
    const focusedIndex = [...document.querySelectorAll('.installed-app')].findIndex(row => row.dataset.appId === focusedId);
    $('content').innerHTML = state.visibleApps.length
      ? `${refreshError ? `<p class="my-apps-refresh-error" role="alert">App refresh failed. Showing the last loaded records. <button class="text-button" data-action="retry">Try again</button></p>` : ''}<div class="my-apps-layout"><div class="installed-list" role="group" aria-label="Saved apps">${state.visibleApps.map((app, index) => installedRow(app, index, app.id === selectedAppId)).join('')}</div><aside id="my-apps-detail" class="my-apps-detail" aria-label="Selected app details">${selectedDetail(state.visibleApps.find(app => app.id === selectedAppId), detailTab, detailLog)}</aside></div>`
      : emptyState(state.query ? 'No matching apps' : 'Your apps belong here.', state.query ? 'Try another name or address.' : 'Install a reviewed recipe or connect an app you already run.', state.query ? 'clear' : 'discover', state.query ? 'Clear search' : 'Discover apps');
    paintOperations();
    if (focusedId) {
      const rows = [...document.querySelectorAll('.installed-app')];
      const row = rows.find(row => row.dataset.appId === focusedId) || rows[Math.min(focusedIndex, rows.length - 1)];
      ((focusedInDetail ? $('my-apps-detail')?.querySelector(`[data-${focusedAction}]`) || $('my-apps-detail')?.querySelector('.primary') : null) || row?.querySelector(`[data-${focusedAction}]`) || row?.querySelector('.primary') ||
        $('content').querySelector('.empty-state button'))?.focus({preventScroll: true});
    }
    return;
  }
  loadState(true);
  // Keep the previous results in place during search; avoid a loading flash on every key.
  if (!$('content').children.length) $('content').innerHTML = loadingComposition('Finding your next app…');
  try {
    const page = await invoke('search_catalog', { query: state.query, category: state.category, offset: state.offset, limit: state.limit, filters: state.filters });
    if (token !== request) return;
    let facts = [];
    try { facts = await invoke('launch_readiness_batch', {ids: page.entries.filter(app => app.recipe_id).map(app => app.recipe_id)}) || []; } catch { /* A proof failure never promotes a listing. */ }
    if (token !== request) return;
    page.entries = page.entries.map(app => ({...app, snapshot_date:page.snapshot_date, launch_readiness: facts.find(item => item.offering_id === app.recipe_id)}));
    state.entries = append ? [...state.entries, ...page.entries] : page.entries;
    state.total = page.total;
    controls.update(page);
    $('results-count').textContent = `${page.total.toLocaleString()} ${page.total === 1 ? 'project' : 'projects'}`;
    $('catalog-note').textContent = `${page.catalog_total.toLocaleString()} projects to discover · Works offline · Install previews are marked`;
    $('content').innerHTML = state.entries.length
      ? `<div class="app-grid">${state.entries.map(discoveryCard).join('')}</div>`
      : emptyState('Nothing here just yet.', 'Try a different search or category. You can also connect an app that isn’t in this collection.', 'clear', 'Clear filters', 'search');
    $('pagination').hidden = page.total === 0;
    const narrowed = state.query || state.category || Object.values(state.filters).some(Boolean);
    $('page-label').textContent = `Showing ${state.entries.length.toLocaleString()}${narrowed ? ' matching' : ''} projects${state.entries.length < page.total ? ` of ${page.total.toLocaleString()}` : ''}`;
    $('next').hidden = state.entries.length >= page.total;
    $('next').disabled = false;
    $('next').textContent = 'Load next 24';
  } catch (error) { if (token === request) { if (append) toast('Could not load more apps. Your current results are still here. Try again.'); else showError(error); } }
  finally { if (token === request) { loadState(false); $('next').disabled = false; $('next').textContent = 'Load next 24'; } }
}
function showError(error) { $('content').innerHTML = emptyState('We couldn’t load your workspace.', message(error), 'retry', 'Try again', 'circle-alert'); }
function navigate(view, updateHistory = true) {
  if (settingsScreen.dataset.busy === 'true' && state.view === 'settings' && view !== 'settings') return;
  if (view === 'settings' && state.view !== 'settings') beforeSettings = state.view;
  if (settingsScreen.open && view !== 'settings') settingsScreen.close();
  if (updateHistory && location.hash !== `#${view === 'apps' ? 'my-apps' : view}`) history.pushState(null,'',`#${view === 'apps' ? 'my-apps' : view}`);
  clearTimeout(searchTimer);
  state.view = view; state.query = ''; state.category = ''; state.offset = 0;
  readiness.update([], false);
  $('search').value = ''; controls.reset();
  const settings = view === 'settings';
  document.querySelector('.workspace').classList.toggle('settings-active',settings);
  if (settings && !settingsScreen.open) settingsScreen.show();
  const overview = view === 'overview';
  const activity = view === 'activity';
  $('activity-screen').hidden = !activity;
  document.querySelector('.workspace').classList.toggle('activity-active', activity);
  $('overview-screen').hidden = !overview;
  document.querySelector('.workspace').classList.toggle('overview-active', overview);
  const discover = view === 'discover';
  for (const [id, active] of [['nav-overview', overview], ['nav-discover', discover], ['nav-apps', view === 'apps'], ['nav-activity', activity], ['settings', settings]]) {
    $(id).classList.toggle('selected', active);
    if (active) $(id).setAttribute('aria-current', 'page'); else $(id).removeAttribute('aria-current');
  }
  $('breadcrumb').textContent = settings ? 'Settings' : overview ? 'Overview' : activity ? 'Activity' : discover ? 'Discover' : 'My Apps';
  if (settings) { $('settings-title').focus({preventScroll:true}); window.scrollTo({top:0,behavior:'instant'}); void refreshEngineStatus(); void refreshLaunchCollection(); return; }
  if (activity) { renderActivity(); $('activity-title').focus({preventScroll: true}); window.scrollTo({top: 0, behavior: 'instant'}); return; }
  if (overview) {
    $('overview-content').innerHTML = loadingComposition('Checking your apps and local engine…');
    $('overview-title').focus({preventScroll: true});
    void refreshOverview();
    window.scrollTo({top: 0, behavior: 'instant'});
    return;
  }
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
  $('page-title').focus({preventScroll:true});
}
let overviewRequest = 0;
async function refreshOverview() {
  const token = ++overviewRequest;
  const button = $('overview-refresh');
  button.disabled = true;
  button.textContent = 'Checking…';
  $('overview-content').setAttribute('aria-busy', 'true');
  const [appsResult, engineResult] = await Promise.allSettled([invoke('list_apps'), invoke('managed_engine_status')]);
  if (token !== overviewRequest || state.view !== 'overview') return;
  publishEngineStatus(engineResult.status === 'fulfilled' ? engineResult.value : null, engineResult.status === 'rejected');
  $('overview-content').setAttribute('aria-busy', 'false');
  if (appsResult.status === 'fulfilled') {
    state.apps = appsResult.value;
    refreshError = null;
    $('app-count').textContent = state.apps.length;
  } else {
    refreshError = appsResult.reason;
    $('app-count').textContent = '–';
  }
  renderOverview(state.apps, engineResult.status === 'fulfilled' ? engineResult.value : null,
    refreshError, engineResult.status === 'rejected' ? engineResult.reason : null);
  button.disabled = false;
  button.textContent = 'Refresh status';
}
function openConnect(name = '', catalogId = null, starter = false) {
  starterConnection = starter;
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
async function reviewInstall(recipeId, starterPort = null) {
  const token = ++recipeRequest;
  starterReview = starterPort !== null;
  markStarterStep(starterReview ? 'install' : null, $('install-dialog'));
  $('install-dialog').querySelector('.modal-top button').setAttribute('aria-label', starterReview ? 'Back to starter choices' : 'Back to Discover');
  activeRecipe = null;
  installNeedsReview = false;
  resetInstallTask();
  $('install-title').textContent = 'Review installation';
  $('install-content').innerHTML = loadingComposition('Checking local engine and recipe…', 'install');
  $('install-error').textContent = '';
  $('install-confirm').textContent = 'Checking system…';
  $('install-confirm').disabled = true;
  showDialog($('install-dialog'));
  try {
    const [recipe, report, facts] = await Promise.all([invoke('recipe_details', { id: recipeId }), invoke('doctor'), invoke('launch_readiness', {id: recipeId}).catch(() => null)]);
    if (token !== recipeRequest || !$('install-dialog').open) return;
    activeRecipe = recipe;
    $('install-title').textContent = `Review ${recipe.display_name} installation`;
    $('install-confirm').textContent = `Install ${recipe.display_name}`;
    $('install-confirm').disabled = !report.ready;
    $('install-content').innerHTML = recipeView(recipe, report, facts);
    wirePortChoice(recipe);
    if (starterPort !== null && $('recipe-port')) {
      if ($('port-field').hidden) $('change-port').click();
      $('recipe-port').value = starterPort;
      $('recipe-port').dispatchEvent(new Event('input', {bubbles:true}));
    }
    // Rebuilt from the projection every time the review opens, so nothing a
    // previous review was given survives into this one.
    renderSetupFields($('install-content'), recipe.setup_review);
    if (!report.ready) {
      $('install-error').textContent = 'Set up or select the Local Store engine in Settings, then reopen this review.';
      const setup = document.createElement('button'); setup.className = 'secondary'; setup.id = 'install-engine-settings'; setup.textContent = 'Set up local engine';
      setup.onclick = async () => { starterReview = false; await closeDialog($('install-dialog')); $('settings').click(); };
      $('install-content').append(setup);
    }
  } catch (error) {
    if (token !== recipeRequest || !$('install-dialog').open) return;
    $('install-content').innerHTML = '';
    $('install-error').textContent = diagnosticText(error);
    $('install-confirm').textContent = 'Install unavailable';
  }
}
initializeGithubReview({reviewInstall});
initializeFirstRun({reviewInstall, connectExisting:() => openConnect('',null,true)});
$('connect-dialog').addEventListener('close', () => { if (starterConnection) {starterConnection = false; void showFirstRun(true);} });
$('install-dialog').addEventListener('close', () => {if (starterReview) {starterReview = false; void returnToStarterChoice();}});
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
async function refreshSelectedLogs() {
  const app = state.visibleApps.find(item => item.id === selectedAppId);
  if (!app || app.runtime?.kind !== 'compose') return;
  const appId = app.id;
  detailLog = {state:'loading'};
  $('my-apps-detail').innerHTML = selectedDetail(app, 'logs', detailLog);
  try {
    const text = await invoke('app_logs', {id: appId});
    if (state.view !== 'apps' || selectedAppId !== appId || detailTab !== 'logs') return;
    detailLog = {state:'ready', text: text || ''};
  } catch (error) {
    if (state.view !== 'apps' || selectedAppId !== appId || detailTab !== 'logs') return;
    detailLog = {state:'error', error: message(error)};
  }
  $('my-apps-detail').innerHTML = selectedDetail(app, 'logs', detailLog);
}
$('nav-discover').onclick = () => navigate('discover');
$('nav-overview').onclick = () => navigate('overview');
$('nav-activity').onclick = () => navigate('activity');
$('nav-agents').onclick = async () => { await showDialog($('settings-dialog')); await refreshAgents(); $('agent-client-name')?.focus(); };
$('nav-apps').onclick = async () => { navigate('apps'); $('content').innerHTML = loadingComposition('Loading your saved apps…', 'apps'); loadState(true); await refreshApps(); if (state.view === 'apps') render(); };
$('overview-refresh').onclick = refreshOverview;
document.querySelector('.brand').onclick = event => { event.preventDefault(); navigate('discover'); };
$('connect-top').onclick = $('connect-note').onclick = () => openConnect();
$('about').onclick = () => showDialog($('about-dialog'));
$('settings').onclick = $('rail-engine').onclick = () => navigate('settings');
window.addEventListener('local-store:settings-route',event => { navigate(event.detail.opening ? 'settings' : beforeSettings); });
const routeFromHash = () => { const route = location.hash.slice(1); const view = route === 'my-apps' ? 'apps' : route; if (['overview','discover','apps','activity','settings'].includes(view)) navigate(view,false); };
window.addEventListener('popstate',() => {routeFromHash(); const restoredView=state.view; setTimeout(() => {if(state.view!==restoredView || document.querySelector('dialog:modal')) return; $(restoredView==='settings'?'settings-title':restoredView==='overview'?'overview-title':restoredView==='activity'?'activity-title':'page-title')?.focus({preventScroll:true});},0);});
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
$('next').onclick = () => { if ($('content').getAttribute('aria-busy') === 'true') return; state.offset = state.entries.length; $('next').disabled = true; $('next').textContent = 'Loading…'; render(true); };
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && state.view === 'settings' && !document.querySelector('dialog:modal')) { event.preventDefault(); navigate(beforeSettings); return; }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k' && ['discover','apps'].includes(state.view) && !document.querySelector('dialog[open]')) { event.preventDefault(); $('search').focus(); }
  if (state.view !== 'apps' || document.querySelector('dialog[open]')) return;
  const selector = event.target.closest?.('.installed-select');
  if (selector && ['ArrowUp','ArrowDown','Home','End'].includes(event.key)) {
    const buttons = [...document.querySelectorAll('.installed-select')];
    const index = buttons.indexOf(selector);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1
      : Math.max(0, Math.min(buttons.length - 1, index + (event.key === 'ArrowDown' ? 1 : -1)));
    event.preventDefault(); buttons[next]?.click(); buttons[next]?.focus();
  }
  const tab = event.target.closest?.('[data-my-apps-tab]');
  if (tab && ['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) {
    const tabs = [...document.querySelectorAll('[data-my-apps-tab]')];
    const index = tabs.indexOf(tab);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1
      : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
    event.preventDefault(); tabs[next]?.click();
  }
});
document.addEventListener('error', event => { if (event.target.matches?.('.app-avatar img')) event.target.remove(); }, true);
document.addEventListener('click', async event => {
  const button = event.target.closest('button');
  if (!button || button.getAttribute('aria-disabled') === 'true') return;
  if (button.dataset.selectApp !== undefined) {
    const app = state.visibleApps[Number(button.dataset.selectApp)];
    if (!app) return;
    selectedAppId = app.id; detailTab = 'overview'; detailLog = {state:'idle'};
    document.querySelectorAll('.installed-app').forEach(row => {
      const selected = row.dataset.appId === selectedAppId;
      row.classList.toggle('selected', selected);
      row.querySelector('.installed-select').setAttribute('aria-pressed', String(selected));
    });
    $('my-apps-detail').innerHTML = selectedDetail(app, detailTab, detailLog);
    paintOperations();
    return;
  }
  if (button.dataset.myAppsTab) {
    detailTab = button.dataset.myAppsTab;
    const app = state.visibleApps.find(item => item.id === selectedAppId);
    if (!app) return;
    $('my-apps-detail').innerHTML = selectedDetail(app, detailTab, detailLog);
    $('my-apps-detail').querySelector(`[data-my-apps-tab="${detailTab}"]`)?.focus();
    paintOperations();
    if (detailTab === 'logs' && detailLog.state === 'idle') void refreshSelectedLogs();
    return;
  }
  if (button.dataset.myAppsLog === 'copy') {
    const appId = selectedAppId, original = detailLog;
    if (detailLog.state !== 'ready') return;
    try {
      if (!navigator.clipboard?.writeText) throw new Error('Clipboard is unavailable. Select the log text and copy it manually.');
      await navigator.clipboard.writeText(detailLog.text || '');
      if (selectedAppId === appId && detailLog === original) detailLog = {...original, copyStatus:'Logs copied. They may contain private app data.'};
    } catch (error) {
      if (selectedAppId === appId && detailLog === original) detailLog = {...original, copyStatus:error.message || 'Could not copy logs. Select the text and copy it manually.'};
    }
    if (selectedAppId === appId && detailTab === 'logs') {
      const restoreFocus = document.activeElement === button;
      $('my-apps-detail').innerHTML = selectedDetail(state.visibleApps.find(app => app.id === appId), 'logs', detailLog);
      if (restoreFocus) $('my-apps-detail').querySelector('[data-my-apps-log="copy"]')?.focus();
    }
    return;
  }
  if (button.dataset.myAppsLog === 'refresh') { void refreshSelectedLogs(); return; }
  if (button.dataset.overview) {
    if (button.dataset.overview === 'refresh') void refreshOverview();
    else if (button.dataset.overview === 'settings') $('settings').click();
    else if (button.dataset.overview === 'activity') navigate('activity');
    else if (button.dataset.overview === 'apps') { await refreshApps(); navigate('apps'); }
    else navigate('discover');
    return;
  }
  if (button.dataset.projectUrl) { try { await invoke('open_project', { url: button.dataset.projectUrl }); } catch (error) { $('detail-error').textContent = message(error); } return; }
  if (button.dataset.close) { await closeDialog($(button.dataset.close)); return; }
  if (button.dataset.featured) { reviewInstall(button.dataset.featured); return; }
  if (button.dataset.catalogAction !== undefined) {
    const app = state.entries[Number(button.dataset.catalogAction)]; if (!app) return;
    if (app.capability === 'preview_install') reviewInstall(app.recipe_id);
    else if (app.capability === 'discover') {try {await invoke('open_project',{url:app.website_url || app.source_url});} catch(error) {toast(message(error));}}
    else openConnect(app.name,app.id);
    return;
  }
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
    pendingApp = app; $('delete-data').checked = false; $('uninstall-confirm').textContent = 'Uninstall, keep data'; $('delete-name').value = ''; $('delete-name-field').hidden = true; $('delete-name-help').hidden = true; $('uninstall-confirm').disabled = false;
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
    const completedStarterConnection = starterConnection; starterConnection = false;
    setDialogBusy($('connect-dialog'), false);
    await closeDialog($('connect-dialog'));
    await refreshApps();
    if (!refreshError) selectedAppId = state.apps.find(app => app.display_name === name)?.id || selectedAppId;
    navigate('apps'); toast(`${name} added to My Apps.`);
    if (completedStarterConnection) { void finishStarterIntroduction(); await showConnectionResult(name, refreshError ? null : state.apps.find(app => app.display_name === name && app.launch_url === rawUrl && app.runtime?.kind === 'external')); }
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
  resetInstallTask();
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
    const completedStarter = starterReview;
    if (starterReview) {starterReview = false; void finishStarterIntroduction();}
    setDialogBusy($('install-dialog'), false);
    await closeDialog($('install-dialog'));
    await refreshApps(); selectedAppId = recipe.id; navigate('apps');
    await showInstallResult(recipe, refreshError ? null : state.apps.find(app => app.id === recipe.id), completedStarter);
  } catch (error) {
    const text = diagnosticText(error);
    $('install-error').textContent = text;
    markInvalidAnswer(text);
    installNeedsReview = retryIsUnsafe(error);
    renderInstallFailure(error);
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
$('install-outcome-recovery').onclick=async()=>{starterReview=false;await closeDialog($('install-dialog'));$('settings').click();$('scan-recovery').click();};
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
function updateDeleteConfirmation() {
  const deleting = $('delete-data').checked;
  $('delete-name-field').hidden = !deleting; $('delete-name-help').hidden = !deleting;
  $('delete-name-help').textContent = pendingApp ? `Enter exactly: ${pendingApp.display_name}` : '';
  $('uninstall-confirm').textContent = deleting ? 'Uninstall and delete data' : 'Uninstall, keep data';
  $('uninstall-confirm').disabled = deleting && $('delete-name').value !== pendingApp?.display_name;
}
$('delete-data').onchange = () => { $('delete-name').value = ''; updateDeleteConfirmation(); if ($('delete-data').checked) $('delete-name').focus(); };
$('delete-name').oninput = updateDeleteConfirmation;
$('uninstall-dialog').addEventListener('close', () => { $('delete-name').value = ''; });
$('uninstall-confirm').onclick = async () => {
  const app = pendingApp, deleteData = $('delete-data').checked;
  if (!app || $('uninstall-dialog').dataset.busy === 'true' || (deleteData && $('delete-name').value !== app.display_name)) return;
  $('delete-data').disabled = true; $('delete-name').disabled = true;
  $('uninstall-confirm').disabled = true; setDialogBusy($('uninstall-dialog'), true);
  try {
    await invokeOperation('uninstall', app.id, 'uninstall_app', { id: app.id, deleteData });
    setDialogBusy($('uninstall-dialog'), false); await closeDialog($('uninstall-dialog'));
    await refreshApps(); render(); toast(deleteData ? 'App and managed data deleted.' : 'App uninstalled. Managed data was preserved.');
  } catch (error) { $('uninstall-error').textContent = diagnosticText(error); }
  finally { $('delete-data').disabled = false; $('delete-name').disabled = false; setDialogBusy($('uninstall-dialog'), false); updateDeleteConfirmation(); }
};
// Subscribe before actions are enabled. Missing event support must not block IPC.
let unlisten = () => {};
try { unlisten = await listenOperations(event => {operations.receive(event); receiveActivity(event);}); } catch { /* IPC is authoritative. */ }
try { await listenBrowserFailures(error => toast(diagnosticText(error))); } catch { /* A missing channel must not break the launcher. */ }
let unlistenActivation = () => {};
try { unlistenActivation = await listenActivationFailures(error => toast(diagnosticText(error))); } catch { /* Keep ordinary launcher actions available. */ }
document.addEventListener('visibilitychange', paintOperations);
window.addEventListener('local-store:apps-changed', async () => { await refreshApps(); if (state.view === 'apps') render(); });
window.addEventListener('pagehide', () => { readiness.dispose(); unlisten(); unlistenActivation(); }, {once: true});
void refreshRailEngine();
await refreshApps();
render();

void showFirstRun();

if (location.hash) routeFromHash();
