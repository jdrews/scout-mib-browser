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
  logLevel: 'info',
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
