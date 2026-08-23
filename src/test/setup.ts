import { vi } from "vitest";

// Mock Tauri invoke - it doesn't exist in jsdom
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Node >=25 exposes a global `localStorage` stub whose methods only exist when
// --localstorage-file is set; happy-dom inherits it, so getItem/setItem are
// missing and any module that touches localStorage at import time throws.
// Install a Map-backed shim only when the real one is broken (no-op elsewhere).
if (typeof globalThis.localStorage?.getItem !== "function") {
  const store = new Map<string, string>();
  const shim = {
    get length() {
      return store.size;
    },
    key: (i: number) => Array.from(store.keys())[i] ?? null,
    getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
    setItem: (k: string, v: string) => {
      store.set(k, String(v));
    },
    removeItem: (k: string) => {
      store.delete(k);
    },
    clear: () => store.clear(),
  };
  Object.defineProperty(globalThis, "localStorage", { value: shim, configurable: true });
}
