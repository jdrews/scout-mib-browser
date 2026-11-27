import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, screen, waitFor, cleanup } from "@testing-library/svelte";
import ManageMibsPage from "./ManageMibsPage.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { LoadedMib } from "$lib/types";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

const mibs: LoadedMib[] = [
  { mibName: "SNMPv2-MIB", filePath: "/mibs/SNMPv2-MIB", nodeCount: 10, isFallback: false },
  { mibName: "IF-MIB", filePath: "/mibs/IF-MIB", nodeCount: 20, isFallback: false },
];

const unloadedStatus = { nodeCount: 0, fallbackMibs: [] };

// "Unload All" is one click from destroying the whole tree (launch-beta/11):
// it must name the module count and require a second, deliberate click.

describe("ManageMibsPage Unload All confirmation (launch-beta/11)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_loaded_list") return Promise.resolve(mibs);
      if (cmd === "mib_unload_many") return Promise.resolve(unloadedStatus);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "mib_tree") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    S.manageMibsOpen = true;
    S.statusText = "Ready";
  });

  afterEach(() => {
    cleanup();
    S.manageMibsOpen = false;
    S.statusText = "Ready";
  });

  async function renderOpenPage() {
    render(ManageMibsPage);
    await waitFor(() => expect(screen.getByTestId("mibs-table")).toBeTruthy());
  }

  it("the first click asks for confirmation naming the module count", async () => {
    await renderOpenPage();
    await fireEvent.click(screen.getByTestId("unload-all-btn"));
    const bar = await screen.findByTestId("unload-all-confirm-bar");
    expect(bar.textContent).toContain("2");
    // Nothing has been unloaded yet.
    expect(mockedInvoke).not.toHaveBeenCalledWith("mib_unload_many", expect.anything());
  });

  it("Cancel keeps every module loaded", async () => {
    await renderOpenPage();
    await fireEvent.click(screen.getByTestId("unload-all-btn"));
    await fireEvent.click(await screen.findByTestId("unload-all-cancel-btn"));
    // The bar is gone, the table is intact, and no unload was invoked.
    await waitFor(() =>
      expect(screen.queryByTestId("unload-all-confirm-bar")).toBeNull(),
    );
    expect(screen.getByTestId("mibs-table")).toBeTruthy();
    expect(mockedInvoke).not.toHaveBeenCalledWith("mib_unload_many", expect.anything());
    expect(S.statusText).toBe("Ready");
  });

  it("Confirm unloads every module by path", async () => {
    let unloaded = false;
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_loaded_list") return Promise.resolve(unloaded ? [] : mibs);
      if (cmd === "mib_unload_many") {
        unloaded = true;
        return Promise.resolve(unloadedStatus);
      }
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "mib_tree") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    await renderOpenPage();
    await fireEvent.click(screen.getByTestId("unload-all-btn"));
    await fireEvent.click(await screen.findByTestId("unload-all-confirm-btn"));

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("mib_unload_many", {
        filePaths: ["/mibs/SNMPv2-MIB", "/mibs/IF-MIB"],
      }),
    );
    await waitFor(() => expect(S.statusText).toBe("Unloaded all MIBs"));
  });
});

// axe (launch-beta/13): the page replaces the MIB Browser + Results view,
// which lives inside <main> — the page must carry that landmark itself, or
// the whole Manage MIBs state has no main landmark (landmark-one-main) and
// its controls sit outside every region.
describe("ManageMibsPage main landmark (launch-beta/13)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_loaded_list") return Promise.resolve(mibs);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "mib_tree") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    S.manageMibsOpen = true;
  });

  afterEach(() => {
    cleanup();
    S.manageMibsOpen = false;
  });

  it("the page root is the main landmark", async () => {
    render(ManageMibsPage);
    const page = await screen.findByTestId("manage-mibs-page");
    expect(page.tagName).toBe("MAIN");
  });
});
