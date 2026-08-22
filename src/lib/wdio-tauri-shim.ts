// Test support: Tauri v2 does not inject the legacy `window.__TAURI__` global in
// this build, but @wdio/tauri-plugin (imported for e2e mocking/window management)
// looks for `window.__TAURI__.core.invoke`. Expose a minimal shim backed by the
// real IPC bridge so the plugin can install its invoke interception. Must be
// imported before "@wdio/tauri-plugin".
const w = window as unknown as {
  __TAURI__?: { core?: { invoke?: unknown } };
  __TAURI_INTERNALS__?: { invoke?: (cmd: string, args?: unknown) => Promise<unknown> };
};
if (!w.__TAURI__ && w.__TAURI_INTERNALS__?.invoke) {
  const internalInvoke = w.__TAURI_INTERNALS__.invoke.bind(w.__TAURI_INTERNALS__);
  w.__TAURI__ = { core: { invoke: (cmd: string, args?: unknown) => internalInvoke(cmd, args) } };
}
