import { defineConfig } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
// Windows keeps browser scratch files on the workspace drive. On POSIX, use
// the OS temp directory: Chromium's Unix socket path has a short length limit
// and a nested checkout (including GitHub's owner/repo paths) can exceed it.
if (process.platform === 'win32') {
  const temporaryDirectory = fileURLToPath(new URL('./.cache/playwright-tmp/', import.meta.url));
  mkdirSync(temporaryDirectory, {recursive:true});
  process.env.TMPDIR = process.env.TMP = process.env.TEMP = temporaryDirectory;
}
export default defineConfig({
  testDir: './tests/ui', snapshotPathTemplate: '{testDir}/{testFilePath}-snapshots/{arg}{ext}',
  // The unreleased Windows exact-byte capture gate has a separate configuration.
  // Keep its known renderer failure visible there, without treating it as a
  // passing gate or breaking the source milestone's functional regression lane.
  testIgnore: ['**/v2-local-acceptance.spec.js'],
  // The suite finishes in about 50 seconds on an idle machine, but these
  // tests share a runner with compilation and other checks. At 20 seconds the
  // keyboard/accessibility test timed out on every run launched alongside the
  // Rust and Python gates (28.9s observed) while passing every idle run (4.5s).
  // The timeout is here to catch a hang, and 60 seconds still does that.
  timeout: 60_000, fullyParallel: true,
  use: { baseURL: 'http://127.0.0.1:4173', viewport: { width: 1280, height: 800 }, trace: 'retain-on-failure', channel: 'chromium' },
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.01 } },
  webServer: { command: 'node scripts/preview.mjs', url: 'http://127.0.0.1:4173', reuseExistingServer: true }
});

