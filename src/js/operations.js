export const installStageLabel = stage => ({
  checking_system: 'Checking Docker and Compose…',
  preparing_files: 'Preparing app storage…',
  validating_recipe: 'Validating the app configuration…',
  starting_containers: 'Downloading images and starting the app…',
  waiting_for_health: 'Waiting for the app to respond…',
  saving_app: 'Saving to My Apps…',
  rolling_back: 'Cleaning up the incomplete setup…',
})[stage];
// Local requests own busy state. Events enrich it; IPC always settles it,
// including when notifications are delayed, unavailable or out of order.
export const operationLabel = kind => ({start: 'Starting…', stop: 'Stopping…', open: 'Opening…', install: 'Installing…', uninstall: 'Uninstalling…', shortcut: 'Creating shortcut…'})[kind] || 'Working…';
export function diagnosticText(error) {
  if (!error) return '';
  const hint = {
    port_in_use: 'Choose an unused port or stop the app using it before trying again.',
    prerequisite_unavailable: 'Set up or select the Local Store engine in Settings, then run the system check.',
    process_unavailable: 'Open Settings and run the system check.',
    timed_out: 'Check the app status and logs before trying again.',
    operation_busy: 'Wait for the current operation to finish.',
    storage_io: 'Check access to the app storage folder.',
    rollback_failed: 'Local Store could not finish cleaning up, so containers or files may remain. Review them before installing again.',
    cancelled: 'Nothing was added to My Apps.',
    already_exists: 'Open My Apps to see the existing entry.',
  }[error.code];
  return [error.message || String(error), hint].filter(Boolean).join(' ');
}
// Stages at or past the registry commit. The backend ignores a cancel from
// here on, so offering one would promise something it will not do.
const UNCANCELLABLE_STAGES = new Set(['saving_app', 'rolling_back']);
// After a failed cleanup the machine's state is unknown: containers may still
// be up and files may remain. Offering the same button again invites a blind
// retry over that wreckage, so the dialog stops offering one.
export const retryIsUnsafe = error => error?.code === 'rollback_failed';

// A cancel is offered only while a known operation is still doing cancellable
// work. `finishing` means a terminal event arrived and IPC is settling.
export const canCancel = entry =>
  Boolean(entry?.pending) && !entry.finishing && entry.cancelId != null && !UNCANCELLABLE_STAGES.has(entry.stage);

export function createOperations(changed = () => {}) {
  const entries = new Map();
  return {
    get: id => entries.get(id),
    begin(id, kind) {
      if (entries.get(id)?.pending) return null;
      const token = Symbol(id);
      entries.set(id, {token, kind, pending: true, finishing: false, error: null});
      changed(id); return token;
    },
    receive(event) {
      const entry = entries.get(event.app_id);
      if (!entry?.pending || entry.kind !== event.kind) return;
      if (event.state === 'started') {
        if (!entry.operationId) { entry.operationId = event.operation_id; entry.cancelId = event.cancel_id ?? null; }
      } else if (entry.operationId === event.operation_id) {
        if (event.state === 'progress') {
          if (!entry.finishing && installStageLabel(event.stage)) entry.stage = event.stage;
        } else entry.finishing = true;
      }
      changed(event.app_id);
    },
    // Records that a cancel was requested, so the control stops offering
    // itself while the backend unwinds. IPC still settles the operation.
    cancelRequested(id) {
      const entry = entries.get(id);
      if (!entry?.pending) return;
      entry.cancelId = null;
      entry.cancelRequested = true;
      changed(id);
    },
    finish(id, token, error = null) {
      const entry = entries.get(id);
      if (entry?.token !== token) return;
      if (error) entries.set(id, {pending: false, error});
      else entries.delete(id);
      changed(id);
    },
  };
}
