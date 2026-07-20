import AppShell from "./lib/components/AppShell.svelte";
import "./app.css";

const app = new AppShell({
  target: document.getElementById("app")!,
});

export default app;
