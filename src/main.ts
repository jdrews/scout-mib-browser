import { mount } from "svelte";
import AppShell from "./lib/components/AppShell.svelte";
import "./lib/wdio-tauri-shim";
import "@wdio/tauri-plugin";
import "./app.css";

const app = mount(AppShell, {
  target: document.getElementById("app")!,
});

export default app;
