import { S } from "./stores.svelte";
import { persistTargetConfig } from "./tauriCommands";
import { debounce } from "./debounce";
import type { TargetConfig } from "./types";

// Target settings used to persist on every keystroke, un-awaited and
// un-caught: a failing write was invisible, and each character fired an IPC
// write (launch-beta/10). All keystroke paths go through this debounced,
// error-reporting persister; explicit Save/Connect paths keep calling
// persistTargetConfig directly.

const schedulePersist = debounce((cfg: TargetConfig) => {
  persistTargetConfig({ ...cfg }).catch((err) => {
    S.statusText = `Failed to save target settings: ${err}`;
    console.error("Failed to persist target config:", err);
  });
}, 300);

/** Queue a debounced persist of the given target config (~300 ms). */
export function schedulePersistTargetConfig(cfg: TargetConfig): void {
  schedulePersist({ ...cfg });
}
