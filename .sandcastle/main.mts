import { run, opencode } from "@ai-hero/sandcastle";
import { podman } from "@ai-hero/sandcastle/sandboxes/podman";

// Usage: npx tsx .sandcastle/main.mts [ticket-index]
//   ticket-index defaults to "01", pass "02".."10" for other tickets
const ticketIndex = process.argv[2] || "01";

// Map of ticket index -> filename (for promptArgs substitution)
const TICKETS: Record<string, string> = {
  "01": "01-project-scaffolding.md",
  "02": "02-config-management.md",
  "03": "03-mib-resolver.md",
  "04": "04-snmp-engine.md",
  "05": "05-mib-tree-view.md",
  "06": "06-connection-panel.md",
  "07": "07-snmp-execution-results.md",
  "08": "08-export.md",
  "09": "09-table-retrieval.md",
  "10": "10-system-log.md",
};

const ticketName = TICKETS[ticketIndex];
if (!ticketName) {
  throw new Error(`Unknown ticket index "${ticketIndex}". Available: ${Object.keys(TICKETS).join(", ")}`);
}

await run({
  agent: opencode("lmstudio/qwen3.6-27b-mtp@q4_k_xl"),
  sandbox: podman({
    imageName: "sandcastle/scout-mib-browser:local",
    containerfile: ".sandcastle/Containerfile",
    mounts: [
      // Mount tickets directory so the agent can read them inside the sandbox
      { hostPath: "~/git/scout-tickets", sandboxPath: "/tickets", readonly: true },
      // Mount opencode config so the agent knows about LM Studio
      { hostPath: "~/git/dev-contained/rootless-podman-sandbox/opencode.json", sandboxPath: "/home/agent/.config/opencode/opencode.json", readonly: true },
    ],
  }),
  promptFile: ".sandcastle/prompt.md",
  promptArgs: {
    TICKET_FILE: `/tickets/scout-mib-browser-mvp/${ticketName}`,
  },
  branchStrategy: { type: "branch", branch: `agent/ticket-${ticketIndex}` },
  maxIterations: 10,
  name: `ticket-${ticketIndex}`,
  hooks: {
    sandbox: {
      onSandboxReady: [
        { command: "rustup default stable" },
        { command: "cargo --version && node --version && opencode --version" },
      ],
    },
  },
  idleTimeoutSeconds: 600,
  completionTimeoutSeconds: 60,
});
