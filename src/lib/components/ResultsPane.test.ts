import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, screen, waitFor, cleanup } from "@testing-library/svelte";
import ResultsPane from "./ResultsPane.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { VariableBinding } from "$lib/types";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

const binding: VariableBinding = {
  oid: "1.3.6.1.2.1.1.1.0",
  value: { OctetString: [...new TextEncoder().encode("router-1")] },
};

// The export is this tool's deliverable: a failed save must reach the
// status bar, not the console (launch-beta/10).

describe("ResultsPane export failures (launch-beta/10)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(undefined);
    S.executionBindings.length = 0;
    S.executionBindings.push(binding);
    S.executionResults = { bindings: [binding], partial: false, warnings: [] };
    S.tableResult = null;
    S.subtreeNodes = null;
    S.statusText = "Ready";
  });

  afterEach(() => {
    cleanup();
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.statusText = "Ready";
  });

  async function exportAsTsv() {
    render(ResultsPane);
    await fireEvent.click(screen.getByTestId("save-btn"));
    await fireEvent.click(screen.getByText("Save as TSV"));
  }

  it("a rejected save reports Export failed with the cause on the status bar", async () => {
    mockedInvoke.mockImplementation((cmd: string) =>
      cmd === "dialog_save_file" ? Promise.reject("Failed to write: disk full") : Promise.resolve(),
    );
    await exportAsTsv();
    await waitFor(() => expect(S.statusText).toContain("Export failed"));
    expect(S.statusText).toContain("disk full");
  });

  it("a successful save leaves no failure message", async () => {
    mockedInvoke.mockResolvedValue("/tmp/scout-export.tsv");
    await exportAsTsv();
    await waitFor(() => expect(mockedInvoke).toHaveBeenCalledWith("dialog_save_file", expect.anything()));
    expect(S.statusText).not.toContain("Export failed");
  });
});
