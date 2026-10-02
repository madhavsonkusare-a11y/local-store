import { escapeHtml, icon } from './render.js';
import { showDialog } from './motion.js';

export const defaultFilters = () => ({ capability: '', license: '', architecture: '', collection: '', hide_warnings: false });
export function catalogControls(state, render) {
  const $ = id => document.getElementById(id);
  let page, draft;
  $('filters-dialog').addEventListener('keydown', event => {
    if (event.key !== 'Tab') return;
    const controls = [...$('filters-dialog').querySelectorAll('button,input,select,a[href]')].filter(control => !control.disabled && control.getClientRects().length);
    const first = controls[0], last = controls.at(-1);
    if (!event.shiftKey && document.activeElement === last) {event.preventDefault(); first?.focus();}
    else if (event.shiftKey && document.activeElement === first) {event.preventDefault(); last?.focus();}
  });
  function categories() {
    const query = $('category-search').value.trim().toLowerCase();
    const facets = page?.category_counts ?? (page?.categories ?? []).map(value => ({ value, count: 0 }));
    const values = [{ value: '', count: facets.reduce((sum, item) => sum + item.count, 0) }, ...facets];
    $('category-options').innerHTML = values.filter(item => !query || (item.value || 'All categories').toLowerCase().includes(query)).map(item => `<button type="button" data-category="${escapeHtml(item.value)}" aria-pressed="${draft.category === item.value}"><span>${escapeHtml(item.value || 'All categories')}</span><span>${item.count.toLocaleString()}</span></button>`).join('') || '<p class="field-hint">No matching categories. Try another word.</p>';
  }
  $('filter-open').onclick = () => {
    const popup = $('filters-dialog'), anchor = $('filter-open').getBoundingClientRect();
    const inset = 16, width = Math.min(330, window.innerWidth - inset*2);
    const height = Math.min(620, window.innerHeight-inset*2);
    const top = Math.max(inset, Math.min(anchor.bottom + 8, window.innerHeight-height-inset));
    popup.style.left = `${Math.max(inset, Math.min(anchor.left, window.innerWidth-width-inset))}px`;
    popup.style.top = `${top}px`;
    popup.style.maxHeight = `${window.innerHeight-top-inset}px`;
    draft = { ...state.filters, category: state.category };
    $('category-search').value = '';
    for (const [id, values, selected, label] of [
      ['filter-license', page?.licenses ?? [], draft.license, 'Any license'],
      ['filter-architecture', page?.architectures ?? [], draft.architecture, 'Any architecture'],
    ]) {
      $(id).replaceChildren(new Option(label, ''), ...values.map(value => new Option(value, value)));
      $(id).value = selected;
    }
    $('filter-warnings').checked = draft.hide_warnings;
    categories(); showDialog($('filters-dialog'));
  };
  $('category-search').oninput = categories;
  $('category-options').onclick = event => {
    const button = event.target.closest('[data-category]');
    if (!button) return;
    draft.category = button.dataset.category;
    for (const item of $('category-options').querySelectorAll('button')) item.setAttribute('aria-pressed', String(item === button));
  };
  $('filter-reset').onclick = () => {
    draft = { ...defaultFilters(), category: '' };
    $('category-search').value = ''; $('filter-license').value = ''; $('filter-architecture').value = ''; $('filter-warnings').checked = false;
    categories();
  };
  $('filters-form').onsubmit = async event => {
    event.preventDefault();
    state.category = draft.category;
    state.filters = { ...draft, license: $('filter-license').value, architecture: $('filter-architecture').value, hide_warnings: $('filter-warnings').checked };
    delete state.filters.category;
    state.offset = 0;
    // Keyboard filtering stays immediate; native dialog restores the trigger.
    $('filters-dialog').close();
    await render();
  };
  function reset() { state.filters = defaultFilters(); state.category = ''; state.offset = 0; }
  $('clear-filters').onclick = () => { reset(); render(); };
  document.addEventListener('click', event => {
    const button = event.target.closest('[data-capability], [data-collection], [data-remove-filter]');
    if (!button) return;
    if (button.dataset.capability !== undefined) state.filters.capability = button.dataset.capability;
    if (button.dataset.collection !== undefined) state.filters.collection = state.filters.collection === button.dataset.collection ? '' : button.dataset.collection;
    if (button.dataset.removeFilter) {
      const key = button.dataset.removeFilter;
      if (key === 'category') state.category = ''; else state.filters[key] = key === 'hide_warnings' ? false : '';
    }
    state.offset = 0; render();
  });
  function update(nextPage) {
    page = nextPage;
    // A debounced search may finish after Filters opens. Keep its facets current
    // without resetting the user's draft category or category-search text.
    if ($('filters-dialog').open) categories();
    const labels = { writing: 'Think & write', automation: 'Make it flow', media: 'Your media', developer: 'Build something' };
    const chips = [
      ['category', state.category], ['license', state.filters.license], ['architecture', state.filters.architecture],
      ['collection', labels[state.filters.collection]], ['hide_warnings', state.filters.hide_warnings ? 'Without source warnings' : ''],
    ].filter(([, value]) => value);
    $('active-filters').innerHTML = chips.map(([key, value]) => `<button data-remove-filter="${key}" aria-label="Remove ${escapeHtml(value)} filter">${escapeHtml(value)}${icon('x')}</button>`).join('');
    $('clear-filters').hidden = !chips.length && !state.filters.capability;
    $('filter-count').hidden = !chips.length;
    $('filter-count').textContent = chips.length;
    for (const button of document.querySelectorAll('[data-capability]')) button.setAttribute('aria-pressed', String(button.dataset.capability === state.filters.capability));
    for (const button of document.querySelectorAll('[data-collection]')) button.setAttribute('aria-pressed', String(button.dataset.collection === state.filters.collection));
    $('settings-catalog').textContent = `${page.catalog_total.toLocaleString()} projects · ${page.source_count ?? 4} upstream collections · Snapshot ${page.snapshot_date ?? '2026-09-05'}. Available artwork is stored locally.`;
  }
  return { reset, update };
}
