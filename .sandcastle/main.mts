import { run, opencode } from "@ai-hero/sandcastle";
import { podman } from "@ai-hero/sandcastle/sandboxes/podman";
import { styleText } from "node:util";

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

const result = await run({
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
        { command: "cargo --version && node --version && opencode --version" },
      ],
    },
  },
  idleTimeoutSeconds: 600,
  completionTimeoutSeconds: 60,
});

// Post-run summary for file logging mode — prints to terminal so you don't
// have to tail the log file.
const iters = result.iterations.length;
const committed = result.commits.length;
const statusColor = result.completionSignal ? "green" : "yellow";
const statusMsg = result.completionSignal
  ? `Agent signaled completion after ${iters} iteration(s).`
  : `Reached max iterations (${iters}) without completion signal.`;

const runName = `ticket-${ticketIndex}`;
console.log("");
console.log(styleText("bold", `[${runName}] Done`));
console.log(styleText(statusColor, statusMsg));
console.log(`  Commits: ${styleText("bold", String(committed))}`);

// Show the agent's summary — text between the last tool call and <promise>COMPLETE</promise>
if (result.completionSignal && result.stdout.includes(result.completionSignal)) {
  const beforeCompletion = result.stdout.split(result.completionSignal)[0];
  const lines = beforeCompletion.trimEnd().split("\n").slice(-12).filter((l) => l.trim());
  if (lines.length > 0) {
    console.log(styleText("dim", "\n--- Agent output ---"));
    for (const line of lines) {
      console.log(line);
    }
  }
}
