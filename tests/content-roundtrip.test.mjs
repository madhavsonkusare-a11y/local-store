import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const script = fileURLToPath(new URL('../scripts/content-roundtrip-probe.mjs', import.meta.url));

async function fixture(kind) {
  const data = { account: null, item: null };
  const server = createServer(async (req, res) => {
    let body = {};
    for await (const part of req) body = JSON.parse(part.toString());
    let status = 200;
    let result = {};
    if (kind === 'memos') {
      if (req.url === '/api/v1/instance/profile') result = { needsSetup: !data.account };
      else if (req.url === '/api/v1/users' && req.method === 'POST') {
        data.account = { username: body.username, password: body.password };
        result = { username: body.username, role: body.role };
      } else if (req.url === '/api/v1/auth/signin') {
        if (body.passwordCredentials?.password !== data.account?.password) status = 401;
        else result = { accessToken: 'a-valid-test-token' };
      } else if (req.url === '/api/v1/memos' && req.method === 'POST') {
        data.item = { name: 'memos/one', content: body.content, visibility: body.visibility };
        result = data.item;
      } else if (req.url === '/api/v1/memos/one') result = data.item;
      else status = 404;
    } else {
      if (req.url === '/api/token') {
        if (body.password !== 'test-password') status = 401;
        else result = { access_token: 'test-token' };
      } else if (req.url === '/api/notes' && req.method === 'POST') {
        data.item = { title: body.title, content: body.content };
        result = data.item;
      } else if (req.url === `/api/notes/${data.item?.title}`) result = data.item;
      else status = 404;
    }
    res.writeHead(status, { 'content-type': 'application/json' });
    res.end(JSON.stringify(result));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return { data, server, url: `http://127.0.0.1:${server.address().port}/` };
}

for (const kind of ['memos', 'flatnotes']) {
  test(`${kind}: exact content survives two verification phases and mutation fails`, async () => {
    const { data, server, url } = await fixture(kind);
    const dir = await mkdtemp(join(tmpdir(), 'local-store-content-'));
    const state = join(dir, 'proof.json');
    const args = [url, state, kind, ...(kind === 'flatnotes' ? ['test-user', 'test-password'] : [])];
    try {
      await run(process.execPath, [script, 'first-use', ...args]);
      const saved = JSON.parse(await readFile(state, 'utf8'));
      assert.match(saved.content, /^Local Store content proof [a-f0-9]{32}$/);
      assert.equal(data.item.content, saved.content);
      await run(process.execPath, [script, 'verify', ...args]);
      await run(process.execPath, [script, 'verify', ...args]);
      data.item.content = 'different content';
      await assert.rejects(run(process.execPath, [script, 'verify', ...args]), /exact content roundtrip failed/);
    } finally {
      await new Promise((resolve) => server.close(resolve));
      await rm(dir, { recursive: true, force: true });
    }
  });
}

test('rejects non-loopback endpoints before sending a request', async () => {
  await assert.rejects(
    run(process.execPath, [script, 'first-use', 'http://example.com/', 'unused.json', 'memos']),
    /unadorned loopback HTTP address/,
  );
});
