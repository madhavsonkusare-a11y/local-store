import {invoke} from './api.js';
import {showDialog, closeDialog} from './motion.js';

const stages = {
  structural_blocker: 'Definition needs changes',
  maintenance_blocker: 'Maintenance review needed',
  needs_source_review: 'Source review needed',
  needs_qualification: 'App verification needed',
};
const text = (tag, value, className) => {
  const node = document.createElement(tag); node.textContent = String(value ?? '');
  if (className) node.className = className;
  return node;
};
// This is only an early input hint. The launcher validates the repository and
// canonical API response before returning any approved offering identity.
function validRepository(value) {
  if (value.length > 2048) return false;
  try {
    const url = new URL(value);
    return url.protocol === 'https:' && url.hostname === 'github.com' && !url.port &&
      !url.username && !url.password && url.pathname.split('/').filter(Boolean).length >= 2;
  } catch { return false; }
}

export function initializeGithubReview({reviewInstall}) {
  if (document.getElementById('github-dialog')) return;
  const entry = text('button', 'Review GitHub link', 'text-button');
  entry.id = 'github-open'; entry.type = 'button'; entry.setAttribute('aria-haspopup', 'dialog');
  document.getElementById('catalog-controls').append(entry);
  const dialog = document.createElement('dialog');
  dialog.id = 'github-dialog'; dialog.className = 'wide-dialog github-review';
  dialog.setAttribute('aria-labelledby', 'github-title');
  dialog.innerHTML = `<form id="github-form"><div class="modal-top"><p class="eyebrow">REVIEW A REPOSITORY</p><button type="button" class="icon-button" id="github-close" aria-label="Close GitHub review"><img src="assets/icons/x.svg" alt=""></button></div><h2 id="github-title">Bring an app closer.</h2><p class="modal-intro">Find a reviewed install from its GitHub link. Checking contacts GitHub for public repository details. No files are downloaded or run.</p><label class="field">GitHub repository<input id="github-url" type="url" maxlength="2048" placeholder="https://github.com/usememos/memos" spellcheck="false" autocomplete="off" required aria-describedby="github-hint github-error"></label><p class="field-hint" id="github-hint">Repository, tree and file links identify the repository. The current default-branch commit is checked; the link does not choose an installation version.</p><p id="github-error" class="form-error" role="alert"></p><div id="github-result" aria-live="polite"></div><div class="modal-actions"><button type="button" class="secondary" id="github-back">Back to Discover</button><button type="submit" class="primary" id="github-check">Check repository</button></div></form>`;
  document.body.append(dialog);
  const input = dialog.querySelector('#github-url'), result = dialog.querySelector('#github-result');
  const error = dialog.querySelector('#github-error'), submit = dialog.querySelector('#github-check');
  let generation = 0;
  const reset = () => { ++generation; result.replaceChildren(); error.textContent = ''; input.removeAttribute('aria-invalid'); input.disabled = false; submit.disabled = false; submit.className = 'primary'; submit.textContent = 'Check repository'; result.removeAttribute('aria-busy'); };
  entry.onclick = async () => { reset(); input.value = ''; await showDialog(dialog); input.focus(); };
  dialog.querySelector('#github-close').onclick = dialog.querySelector('#github-back').onclick = () => closeDialog(dialog);
  // The dialog is created after shared motion initializes, so retain the same
  // Escape and focus behavior explicitly. Closing invalidates in-flight reads.
  dialog.addEventListener('cancel', event => { event.preventDefault(); void closeDialog(dialog); });
  dialog.addEventListener('close', reset);
  input.addEventListener('input', reset);
  async function open(url) {
    try {
      if (!validRepository(url)) throw new Error('This source link is unavailable.');
      await invoke('open_project', {url});
    } catch (failure) { error.textContent = failure?.message || String(failure); }
  }
  function sourceLink(label, url) {
    const button = text('button', label, 'text-button'); button.type = 'button';
    button.onclick = () => open(url); return button;
  }
  function offering(id, name) {
    submit.className = 'secondary';
    const button = text('button', `Review ${name} installation`, 'primary'); button.type = 'button';
    button.onclick = async () => { await closeDialog(dialog); await reviewInstall(id); };
    result.append(button);
  }
  function candidate(row) {
    const section = document.createElement('section'); section.className = 'github-candidate';
    section.append(text('h3', `${row.id} · ${row.source}`), text('p', stages[row.review_stage] || 'Review incomplete', 'github-stage'));
    section.append(text('p', `${row.service_count} ${row.service_count === 1 ? 'service' : 'services'} · ${row.required_inputs} required setup ${row.required_inputs === 1 ? 'answer' : 'answers'}`, 'field-hint'));
    const remaining = document.createElement('ul');
    for (const check of (row.remaining_checks || []).slice(0, 20)) remaining.append(text('li', check));
    if (!remaining.children.length) remaining.append(text('li', 'Complete source review, Windows app-task checks and explicit promotion before installing.'));
    section.append(remaining);
    const provenance = document.createElement('details'); provenance.className = 'provenance';
    provenance.append(text('summary', 'Pinned definition and provenance'));
    provenance.append(text('p', `Definition: ${row.path}`), text('p', `Source commit: ${row.revision}`), text('p', `Source archive SHA-256: ${row.archive_sha256}`));
    if (row.image_references?.length) provenance.append(text('p', `Declared images: ${row.image_references.slice(0, 20).join(', ')}`));
    provenance.append(sourceLink('Open pinned definition', row.definition_url));
    section.append(provenance); result.append(section);
  }
  function paint(inspection) {
    if (!inspection?.resolution || !inspection.canonical_repository || !inspection.commit_sha) throw new Error('Repository inspection returned incomplete details.');
    result.append(text('h3', inspection.canonical_repository.replace('https://github.com/', '')));
    result.append(text('p', `Default branch ${inspection.default_branch} · inspected commit ${inspection.commit_sha.slice(0, 12)}`, 'field-hint'));
    result.append(sourceLink('Open inspected commit', inspection.commit_url));
    if (inspection.requested_repository !== inspection.canonical_repository) result.append(text('p', 'GitHub redirected this link. Only the canonical repository was matched.', 'preview-note'));
    if (inspection.archived) result.append(text('p', 'GitHub marks this repository as archived. Check ongoing support before proceeding.', 'preview-note'));
    const resolution = inspection.resolution;
    if (resolution.kind === 'approved_match') {
      if (!resolution.offering_id || !resolution.display_name) throw new Error('The reviewed app identity is incomplete.');
      result.append(text('p', 'A reviewed Local Store definition is available. Its pinned images, setup and current app proof appear in the installation review. The inspected GitHub commit is metadata, not installation approval.', 'preview-note'));
      offering(resolution.offering_id, resolution.display_name);
    } else if (resolution.kind === 'ambiguous_match') {
      result.append(text('p', 'More than one reviewed definition matches. Choose the app to review; nothing has been installed.', 'preview-note'));
      for (const id of resolution.offering_ids || []) offering(id, id);
    } else if (resolution.kind === 'review_candidates') {
      result.append(text('p', 'Upstream definitions are available for review. They are not installable yet. Matching a repository or parsing a definition does not prove the app works.', 'preview-note'));
      for (const row of (resolution.candidates || []).slice(0, 20)) candidate(row);
    } else if (resolution.kind === 'needs_review') {
      result.append(text('p', 'No reviewed installation matches this repository. It needs a supported definition, source review and Windows app verification before it can be installed.', 'preview-note'));
    } else throw new Error('This repository has an unknown review status.');
  }
  dialog.querySelector('#github-form').onsubmit = async event => {
    event.preventDefault(); reset(); const token = generation; const url = input.value.trim();
    if (!validRepository(url)) { error.textContent = 'Use an HTTPS github.com repository link without credentials or a port.'; input.setAttribute('aria-invalid', 'true'); input.focus(); return; }
    submit.disabled = true; input.disabled = true; submit.textContent = 'Checking GitHub…'; result.setAttribute('aria-busy', 'true');
    try {
      const inspection = await invoke('resolve_github_source', {url});
      if (token !== generation || !dialog.open) return;
      paint(inspection);
    } catch (failure) {
      if (token !== generation || !dialog.open) return;
      result.replaceChildren(); error.textContent = failure?.message || String(failure);
    } finally {
      if (token === generation && dialog.open) { submit.disabled = false; input.disabled = false; submit.textContent = 'Check repository'; result.removeAttribute('aria-busy'); }
    }
  };
}
