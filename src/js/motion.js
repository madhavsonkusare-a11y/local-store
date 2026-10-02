// Native dialogs retain focus trapping, Escape handling, and focus restoration.
// Only occasional pointer interactions animate; navigation and search stay immediate.
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
const transitions = new WeakMap();
let keyboardInput = true;
document.addEventListener('pointerdown', () => { keyboardInput = false; document.documentElement.dataset.input = 'pointer'; }, true);
document.addEventListener('keydown', () => { keyboardInput = true; document.documentElement.dataset.input = 'keyboard'; }, true);

function transition(dialog, opening) {
  const previous = transitions.get(dialog);
  const current = previous ? { opacity: getComputedStyle(dialog).opacity, transform: getComputedStyle(dialog).transform } : null;
  previous?.cancel();
  if (opening && !dialog.open) {
    dialog.showModal();
    const parent = document.getElementById(dialog.dataset.parentNav);
    if (parent) { dialog.dataset.previousCurrent = parent.getAttribute('aria-current') || ''; parent.setAttribute('aria-current','location'); }
  }
  if (!opening && !dialog.open) return Promise.resolve();
  if (keyboardInput || reducedMotion.matches) {
    if (!opening) dialog.close();
    transitions.delete(dialog);
    return Promise.resolve();
  }
  const drawer = dialog.classList.contains('drawer');
  const hidden = { opacity: drawer ? 1 : 0, transform: drawer ? 'translateX(100%)' : 'scale(.96)' };
  const visible = { opacity: 1, transform: drawer ? 'translateX(0)' : 'scale(1)' };
  const tokens = getComputedStyle(document.documentElement);
  const animation = dialog.animate(
    [current || (opening ? hidden : visible), opening ? visible : hidden],
    { duration: parseFloat(tokens.getPropertyValue('--v2-duration-panel')), easing: tokens.getPropertyValue(drawer ? '--v2-ease-drawer' : '--v2-ease-out').trim(), fill: 'both' }
  );
  transitions.set(dialog, animation);
  return animation.finished.then(() => {
    if (transitions.get(dialog) !== animation) return;
    if (!opening) dialog.close();
    transitions.delete(dialog);
    animation.cancel();
  }, () => {});
}
export function showDialog(dialog) { return transition(dialog, true); }
export function closeDialog(dialog) {
  if (dialog.dataset.busy === 'true') return Promise.resolve();
  return transition(dialog, false);
}
export function setDialogBusy(dialog, busy) {
  dialog.dataset.busy = String(busy);
  dialog.setAttribute('aria-busy', String(busy));
  dialog.querySelectorAll('[data-close]').forEach(button => { button.disabled = busy; });
}
document.querySelectorAll('dialog').forEach(dialog => {
  dialog.addEventListener('cancel', event => { event.preventDefault(); closeDialog(dialog); });
  if (dialog.classList.contains('drawer')) dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) void closeDialog(dialog);
  });
});
export function revealToast(element) {
  element.getAnimations().forEach(animation => animation.cancel());
  if (!keyboardInput && !reducedMotion.matches) {
    element.animate([{ opacity: 0, transform: 'translateY(8px)' }, { opacity: 1, transform: 'translateY(0)' }],
      { duration: 250, easing: 'cubic-bezier(.23, 1, .32, 1)' });
  }
}

for (const task of document.querySelectorAll('dialog.focused-task')) task.addEventListener('close', () => { const parent = document.getElementById(task.dataset.parentNav); if (!parent) return; if (task.dataset.previousCurrent) parent.setAttribute('aria-current', task.dataset.previousCurrent); else parent.removeAttribute('aria-current'); });
