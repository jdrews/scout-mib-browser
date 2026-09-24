export const config = {
  runner: 'local',
  specs: ['./test/specs/**/*.spec.ts'],
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
  // Default to 'info' locally (verbose webdriver logging helps debugging).
  // In CI, set WDIO_LOG_LEVEL=error to suppress the per-command webdriver
  // logging and the tauri-service "get_window_states not allowed" warning
  // (the embedded driver doesn't register that command, so it warns on
  // every command — thousands of lines of pure noise).
  logLevel: process.env.WDIO_LOG_LEVEL ?? 'info',
  bail: 0,
  baseUrl: 'http://localhost:4444',
  waitforTimeout: 15000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },
};
