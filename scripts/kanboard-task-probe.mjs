// Managed-engine task proof for the pinned Kanboard JSON-RPC deployment.
// Usage: node kanboard-task-probe.mjs first-use|verify LOOPBACK_URL STATE
import { randomBytes } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [phase, address, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !address || !statePath) {
  throw new Error('first-use|verify LOOPBACK_URL STATE required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('Kanboard proof requires an unadorned loopback HTTP address');
}
const basic = `Basic ${Buffer.from('admin:admin').toString('base64')}`;
let requestId = 0;
async function rpc(method, params = {}) {
  const id = ++requestId;
  const response = await fetch(new URL('/jsonrpc.php', base), {
    method: 'POST', redirect: 'manual', signal: AbortSignal.timeout(15000),
    headers: { 'content-type': 'application/json', authorization: basic },
    body: JSON.stringify({ jsonrpc: '2.0', id, method, params }),
  });
  if (!response.ok) throw new Error(`${method} returned HTTP ${response.status}`);
  const length = Number(response.headers.get('content-length') || 0);
  if (length > 1024 * 1024) throw new Error(`${method} response is too large`);
  const value = await response.json();
  if (!value || value.jsonrpc !== '2.0' || value.id !== id || value.error || !('result' in value)) {
    throw new Error(`${method} returned an invalid JSON-RPC result`);
  }
  return value.result;
}
function positiveId(value) {
  const id = Number(value);
  if (!Number.isSafeInteger(id) || id <= 0) throw new Error('Kanboard returned an invalid identity');
  return id;
}
function columnId(value) {
  const id = Number(value);
  if (!Number.isSafeInteger(id) || id < 0) throw new Error('Kanboard returned an invalid column');
  return id;
}
function assertTask(task, state, column) {
  if (!task || positiveId(task.id) !== state.taskId || positiveId(task.project_id) !== state.projectId ||
      task.title !== state.title || task.description !== state.description ||
      columnId(task.column_id) !== column) {
    throw new Error('Kanboard did not retain the exact task and column');
  }
}
const me = await rpc('getMe');
if (!me || me.username !== 'admin') throw new Error('Kanboard default admin API login failed');
let state;
if (phase === 'first-use') {
  const nonce = randomBytes(16).toString('hex');
  state = { schema: 1, projectName: `Local Store ${nonce}`, title: `Proof task ${nonce}`,
    description: `Exact task body ${nonce}` };
  state.projectId = positiveId(await rpc('createProject', { name: state.projectName }));
  const columns = await rpc('getColumns', { project_id: state.projectId });
  if (!Array.isArray(columns) || columns.length < 2) throw new Error('Kanboard has fewer than two columns');
  const first = positiveId(columns[0].id);
  state.targetColumn = positiveId(columns[1].id);
  if (first === state.targetColumn) throw new Error('Kanboard returned duplicate columns');
  state.taskId = positiveId(await rpc('createTask', {
    project_id: state.projectId, title: state.title, description: state.description, column_id: first,
  }));
  const created = await rpc('getTask', { task_id: state.taskId });
  assertTask(created, state, first);
  const swimlane = columnId(created.swimlane_id);
  if (await rpc('moveTaskPosition', {
    project_id: state.projectId, task_id: state.taskId, column_id: state.targetColumn,
    position: 1, swimlane_id: swimlane,
  }) !== true) throw new Error('Kanboard refused the task move');
} else {
  state = JSON.parse(await readFile(statePath, 'utf8'));
  if (state.schema !== 1 || !/^Local Store [a-f0-9]{32}$/.test(state.projectName) ||
      !/^Proof task [a-f0-9]{32}$/.test(state.title) ||
      !/^Exact task body [a-f0-9]{32}$/.test(state.description) ||
      !Number.isSafeInteger(state.projectId) || !Number.isSafeInteger(state.taskId) ||
      !Number.isSafeInteger(state.targetColumn)) throw new Error('Kanboard proof state is invalid');
}
const project = await rpc('getProjectById', { project_id: state.projectId });
if (!project || project.name !== state.projectName || positiveId(project.id) !== state.projectId) {
  throw new Error('Kanboard did not retain the exact project');
}
const task = await rpc('getTask', { task_id: state.taskId });
assertTask(task, state, state.targetColumn);
if (phase === 'first-use') await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
console.log(`Kanboard project/task ${phase} passed`);
