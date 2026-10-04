import {defineConfig} from '@playwright/test';
import functional from './playwright.config.js';

// Required before F03 acceptance. This gate currently fails on measured native
// renderer anti-alias variance; it is never replaced by a tolerance or skipped.
export default defineConfig({
  ...functional,
  testIgnore: [],
  testMatch: '**/v2-local-acceptance.spec.js',
  workers: 1,
});
