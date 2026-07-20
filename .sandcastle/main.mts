import { createSandbox, opencode } from "@ai-hero/sandcastle";
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

const branch = `agent/ticket-${ticketIndex}`;

const agent = opencode("lmstudio/qwen3.6-27b-mtp@q4_k_xl");

await using sandbox = await createSandbox({
  branch,
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
  hooks: {
    sandbox: {
      onSandboxReady: [
        { command: "cargo --version && node --version && opencode --version" },
      ],
    },
  },
});

// --- Step 1: Implement ---
console.log(styleText("bold", `[ticket-${ticketIndex}] Step 1/3: Implementing...`));
const implResult = await sandbox.run({
  agent,
  promptFile: ".sandcastle/prompt.md",
  promptArgs: {
    TICKET_FILE: `/tickets/scout-mib-browser-mvp/${ticketName}`,
  },
  maxIterations: 10,
  idleTimeoutSeconds: 600,
  completionTimeoutSeconds: 60,
});

// --- Step 2: Code review ---
console.log(styleText("bold", `[ticket-${ticketIndex}] Step 2/3: Code review...`));
const reviewResult = await sandbox.run({
  agent,
  prompt: `Review the changes on this branch and fix any issues you find. Focus on correctness, code quality, and adherence to Rust best practices. Make the fixes directly.\n\nWhen done, signal completion with <promise>COMPLETE</promise>`,
  maxIterations: 5,
  idleTimeoutSeconds: 600,
  completionTimeoutSeconds: 60,
});

// --- Step 3: Format and lint verification ---
console.log(styleText("bold", `[ticket-${ticketIndex}] Step 3/3: Formatting and linting...`));

const fmtResult = await sandbox.exec("cargo fmt --check");
if (fmtResult.exitCode !== 0) {
  console.log(styleText("yellow", "  cargo fmt --check found issues, running cargo fmt to fix..."));
  await sandbox.exec("cargo fmt");
} else {
  console.log(styleText("green", "  cargo fmt: OK"));
}

let lintFixCommits = 0;
for (let attempt = 1; ; attempt++) {
  const clippyResult = await sandbox.exec("cargo clippy -- -D warnings");
  if (clippyResult.exitCode === 0) {
    console.log(styleText("green", "  cargo clippy: OK"));
    break;
  }

  console.log(styleText("yellow", `  cargo clippy found issues (attempt ${attempt}), asking agent to fix...`));
  const fixResult = await sandbox.run({
    agent,
    prompt: `Fix the following cargo clippy errors:\n\n${clippyResult.stdout}\n${clippyResult.stderr}\n\nWhen done, signal completion with <promise>COMPLETE</promise>`,
    maxIterations: 5,
    idleTimeoutSeconds: 600,
    completionTimeoutSeconds: 60,
  });
  lintFixCommits += fixResult.commits.length;

  if (attempt >= 3) {
    console.log(styleText("red", "  cargo clippy still failing after 3 fix attempts."));
    break;
  }
}

// Post-run summary
const totalCommits = implResult.commits.length + reviewResult.commits.length + lintFixCommits;
const statusColor = implResult.completionSignal ? "green" : "yellow";
const statusMsg = implResult.completionSignal
  ? `Implementation completed.`
  : `Implementation reached max iterations without completion signal.`;

const runName = `ticket-${ticketIndex}`;
console.log("");
console.log(styleText("bold", `[${runName}] Done`));
console.log(styleText(statusColor, statusMsg));
console.log(`  Total commits: ${styleText("bold", String(totalCommits))} (impl: ${implResult.commits.length}, review: ${reviewResult.commits.length})`);

// Show the agent's summary from implementation — text after <promise>COMPLETE</promise>, stopping at system noise
if (implResult.completionSignal && implResult.stdout.includes(implResult.completionSignal)) {
  const afterCompletion = implResult.stdout.split(implResult.completionSignal)[1];
  const lines = afterCompletion.trimStart().split("\n");

  // Stop at known system messages
  const stopMarkers = ["Agent stopped", "Collecting commits", "Run complete"];
  let endIdx = lines.length;
  for (let i = 0; i < lines.length; i++) {
    if (stopMarkers.some((m) => lines[i].includes(m))) {
      endIdx = i;
      break;
    }
  }

  const summaryLines = lines.slice(0, endIdx).filter((l) => l.trim());
  if (summaryLines.length > 0) {
    console.log(styleText("dim", "\n--- Agent output ---"));
    for (const line of summaryLines) {
      console.log(line);
    }
  }
}
