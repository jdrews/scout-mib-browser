import { mount } from "svelte";
// IBM Plex pair (docs/specs/2026-11-02-ux-visual-polish.md): sans for UI
// and MIB names, mono for OIDs/values/hex. 400/500/600 — 600 is reserved
// for figures.
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "@fontsource/ibm-plex-mono/600.css";
import AppShell from "./lib/components/AppShell.svelte";
import "./lib/wdio-tauri-shim";
import "@wdio/tauri-plugin";
import "./app.css";

const app = mount(AppShell, {
  target: document.getElementById("app")!,
});

export default app;
