// Opt-in managed-engine proof for the pinned n8n 2.37.10 recipe.
// Usage: node n8n-workflow-probe.mjs first-use|verify LOOPBACK_URL STATE
import { chromium } from '@playwright/test';
import { randomBytes, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

const [phase, address, statePath] = process.argv.slice(2);
if (!['first-use', 'verify'].includes(phase) || !address || !statePath) {
  throw new Error('first-use|verify LOOPBACK_URL STATE required');
}
const base = new URL(address);
if (base.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname) ||
    base.username || base.password || base.pathname !== '/' || base.search || base.hash) {
  throw new Error('n8n proof requires an unadorned loopback HTTP address');
}

const state = phase === 'first-use' ? {
  schema: 1,
  email: `local-store-${randomBytes(8).toString('hex')}@example.invalid`,
  password: `${randomBytes(18).toString('base64url')}Aa1!`,
  name: `Local Store proof ${randomUUID()}`,
  marker: `local-store-result-${randomUUID()}`,
} : JSON.parse(await readFile(statePath, 'utf8'));
if (phase === 'verify' && (state.schema !== 1 ||
    !/^local-store-[a-f0-9]{16}@example\.invalid$/.test(state.email) ||
    !/^Local Store proof [a-f0-9-]{36}$/.test(state.name) ||
    !/^local-store-result-[a-f0-9-]{36}$/.test(state.marker) ||
    !/^[a-zA-Z0-9_-]+$/.test(state.workflowId) ||
    !/^[0-9]+$/.test(state.firstExecutionId))) {
  throw new Error('n8n proof state is invalid');
}

const browser = await chromium.launch({ headless: true, channel: 'msedge' });
try {
  const page = await browser.newPage();
  page.setDefaultTimeout(30000);
  let sessionHeaders;
  page.on('request', request => {
    if (new URL(request.url()).pathname === '/rest/projects/my-projects') {
      const headers = request.headers();
      if (headers['browser-id'] && headers['push-ref']) {
        sessionHeaders = { 'browser-id': headers['browser-id'], 'push-ref': headers['push-ref'] };
      }
    }
  });
  // n8n briefly returns HTTP 200 "starting up" and then a 404 before its editor
  // route is ready. A single navigation can therefore land on a stale 404 page.
  const expectedInput = phase === 'first-use' ? '#email' : '#emailOrLdapLoginId';
  const editorDeadline = Date.now() + 60000;
  while (Date.now() < editorDeadline) {
    await page.goto(base.href, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(500);
    if (await page.locator(expectedInput).isVisible()) break;
  }
  if (!(await page.locator(expectedInput).isVisible())) {
    throw new Error(`n8n did not show its ${phase} editor screen at ${page.url()}: ${(await page.locator('body').innerText()).slice(0, 250)}`);
  }
  if (phase === 'first-use') {
    await page.locator('#email').fill(state.email);
    await page.locator('#firstName').fill('Local');
    await page.locator('#lastName').fill('Store');
    await page.locator('#password').fill(state.password);
    await page.locator('[data-test-id="form-submit-button"]').click();
  } else {
    await page.locator('#emailOrLdapLoginId').fill(state.email);
    await page.locator('#password').fill(state.password);
    await page.locator('[data-test-id="form-submit-button"]').click();
  }
  const deadline = Date.now() + 30000;
  while (!sessionHeaders && Date.now() < deadline) await page.waitForTimeout(250);
  if (!sessionHeaders) throw new Error('n8n did not establish an authenticated editor session');

  async function api(path, method = 'GET', body) {
    const result = await page.evaluate(async ({ path, method, body, sessionHeaders }) => {
      const response = await fetch(path, {
        method, credentials: 'include',
        headers: { ...sessionHeaders, ...(body ? { 'content-type': 'application/json' } : {}) },
        ...(body ? { body: JSON.stringify(body) } : {}),
      });
      const payload = await response.json();
      return { status: response.status, payload };
    }, { path, method, body, sessionHeaders });
    if (result.status !== 200 || !result.payload?.data) {
      throw new Error(`n8n ${method} ${path} returned HTTP ${result.status}`);
    }
    return result.payload.data;
  }

  if (phase === 'first-use') {
    const trigger = 'When clicking Execute workflow';
    const output = 'Set proof marker';
    const workflow = {
      name: state.name,
      nodes: [
        { id: randomUUID(), name: trigger, type: 'n8n-nodes-base.manualTrigger',
          typeVersion: 1, position: [0, 0], parameters: {} },
        { id: randomUUID(), name: output, type: 'n8n-nodes-base.set',
          typeVersion: 3.4, position: [220, 0],
          parameters: { assignments: { assignments: [
            { id: randomUUID(), name: 'proof', value: state.marker, type: 'string' },
          ] }, options: {} } },
      ],
      connections: { [trigger]: { main: [[{ node: output, type: 'main', index: 0 }]] } },
      settings: { executionOrder: 'v1' },
    };
    const created = await api('/rest/workflows', 'POST', workflow);
    if (!/^[a-zA-Z0-9_-]+$/.test(created.id) || created.name !== state.name) {
      throw new Error('n8n did not save the unique workflow');
    }
    state.workflowId = created.id;
  }

  const saved = await api(`/rest/workflows/${state.workflowId}`);
  if (saved.id !== state.workflowId || saved.name !== state.name ||
      saved.nodes?.length !== 2 ||
      saved.nodes[1]?.parameters?.assignments?.assignments?.[0]?.value !== state.marker ||
      saved.connections?.['When clicking Execute workflow']?.main?.[0]?.[0]?.node !== 'Set proof marker') {
    throw new Error('n8n did not retain the exact connected workflow');
  }

  // n8n serializes execution run data with flatted's index-reference format.
  function decodedRunData(serialized) {
    const values = JSON.parse(serialized);
    if (!Array.isArray(values) || values.length > 10000) throw new Error('invalid n8n execution data');
    const memo = new Map();
    function decode(index) {
      if (!Number.isSafeInteger(index) || index < 0 || index >= values.length) throw new Error('invalid execution reference');
      if (memo.has(index)) return memo.get(index);
      const source = values[index];
      if (source === null || typeof source !== 'object') return source;
      const target = Array.isArray(source) ? [] : {};
      memo.set(index, target);
      for (const [key, value] of Object.entries(source)) {
        target[key] = typeof value === 'string' && /^\d+$/.test(value) ? decode(Number(value)) : value;
      }
      return target;
    }
    return decode(0).resultData?.runData;
  }
  async function assertExecution(id) {
    if (!/^[0-9]+$/.test(id)) throw new Error('n8n returned an invalid execution identity');
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) {
      const execution = await api(`/rest/executions/${id}`);
      if (execution.status === 'running' || execution.status === 'new') {
        await page.waitForTimeout(250);
        continue;
      }
      const marker = decodedRunData(execution.data)?.['Set proof marker']?.[0]?.data?.main?.[0]?.[0]?.json?.proof;
      if (execution.workflowId !== state.workflowId || execution.status !== 'success' ||
          execution.finished !== true || marker !== state.marker) {
        throw new Error('n8n execution did not produce the exact workflow result');
      }
      return;
    }
    throw new Error('n8n workflow execution did not finish in time');
  }

  if (phase === 'verify') await assertExecution(state.firstExecutionId);
  const started = await api(`/rest/workflows/${state.workflowId}/run`, 'POST', {
    workflowId: state.workflowId,
    startNodes: [],
    triggerToStartFrom: { name: 'When clicking Execute workflow' },
  });
  await assertExecution(started.executionId);
  if (phase === 'first-use') {
    state.firstExecutionId = started.executionId;
    await writeFile(statePath, JSON.stringify(state), { mode: 0o600, flag: 'wx' });
  }
  console.log(`n8n exact workflow and result ${phase} passed`);
} finally {
  await browser.close();
}
