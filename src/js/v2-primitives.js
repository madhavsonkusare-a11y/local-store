// Adopt the frozen primitives for existing and dynamically rendered controls.
// Observe added nodes only: operation/focus/permission attributes remain owned
// by their existing controllers, and this never replaces an interactive node.
function adopt(root) {
  if (root.nodeType !== Node.ELEMENT_NODE && root.nodeType !== Node.DOCUMENT_NODE) return;
  const buttons = 'button.primary,button.secondary,button.danger';
  if (root.matches?.(buttons)) root.classList.add('button');
  for (const control of root.querySelectorAll(buttons)) control.classList.add('button');
  const fields = 'input:not([type="checkbox"]):not([type="radio"]):not([type="hidden"]),select';
  if (root.matches?.(fields)) root.classList.add('input');
  for (const control of root.querySelectorAll(fields)) control.classList.add('input');
}
adopt(document);
new MutationObserver(records => {
  for (const record of records) for (const node of record.addedNodes) adopt(node);
}).observe(document.body,{childList:true,subtree:true});
