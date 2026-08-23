// UX assessment suite (docs/specs/2026-08-22-ux-assessment.md).
// Separate from wdio.conf.mjs so the probe suite — slow, screenshot-heavy,
// N-times timing loops — never affects the CI feature baseline. Same embedded
// Tauri driver; one shared app instance across the ux spec files (they are
// order-aware: numeric prefixes, each spec reloads for a clean baseline).
export const config = {
  runner: 'local',
  specs: ['./test/specs-ux/**/*.spec.ts'],
  maxInstances: 1,
  services: [
    [
      '@wdio/tauri-service',
      {
        appBinaryPath: './target/debug/scout-mib-browser',
        driverProvider: 'embedded',
        captureBackendLogs: true,
        captureFrontendLogs: true,
        backendLogLevel: 'debug',
        frontendLogLevel: 'debug',
      },
    ],
  ],
  capabilities: [
    {
      browserName: 'tauri',
      'tauri:options': {
        application: './target/debug/scout-mib-browser',
      },
    },
  ],
  logLevel: 'info',
  bail: 0,
  baseUrl: 'http://localhost:4444',
  waitforTimeout: 15000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    // Timing loops (N=5 per metric) and full state scripts are slow by design.
    timeout: 600000,
  },
};
