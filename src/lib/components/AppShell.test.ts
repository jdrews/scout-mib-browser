import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/svelte";
import AppShell from "./AppShell.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

// The global setup mock covers `@tauri-apps/api/core` for our own modules,
// but @tauri-apps/api/app imports core through its own module graph, so the
// app-metadata functions are mocked at their own seam.
const mocks = vi.hoisted(() => ({
  getVersion: vi.fn<() => Promise<string>>(),
}));
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: mocks.getVersion,
}));

const mockedGetVersion = mocks.getVersion;

// Beta feedback triage (launch-beta/12): when a user reports a bug there is
// no way to know which build they ran. The footer must show the app version
// and the System Log must record version + platform at startup.

describe("AppShell version display (launch-beta/12)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedGetVersion.mockReset();
    mockedGetVersion.mockResolvedValue("9.9.9");
    // The platform comes from the webview, not an IPC command.
    vi.stubGlobal("navigator", { platform: "linux" });
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_tree") return Promise.resolve([]);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "config_read") return Promise.resolve({});
      return Promise.resolve(undefined);
    });
    S.statusText = "Ready";
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    S.statusText = "Ready";
  });

  it("the footer shows the app version", async () => {
    render(AppShell);
    const el = await screen.findByTestId("app-version");
    await waitFor(() => expect(el.textContent).toContain("9.9.9"));
  });

  it("startup logs the version and platform to the System Log", async () => {
    render(AppShell);
    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith(
        "log_frontend",
        expect.objectContaining({ level: "INFO", target: "app" }),
      ),
    );
    const call = mockedInvoke.mock.calls.find((c) => c[0] === "log_frontend")!;
    const msg = String((call[1] as { message: string }).message);
    expect(msg).toContain("9.9.9");
    expect(msg).toMatch(/linux/i);
  });

  it("a failed version lookup leaves the footer empty, not broken", async () => {
    mockedGetVersion.mockRejectedValue("no app plugin");
    render(AppShell);
    // Startup completes despite the failed lookup.
    await waitFor(() => expect(screen.getByTestId("status-text")).toBeTruthy());
    // Let any rejected lookup settle before asserting the span is absent.
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.queryByTestId("app-version")).toBeNull();
  });
});

// axe (launch-beta/13): the h1 and the whole target bar sat outside every
// landmark (region violation in every state). The banner landmark is their
// home: the app title and the query controls belong to the page header.
describe("AppShell landmarks (launch-beta/13)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedGetVersion.mockResolvedValue("9.9.9");
    vi.stubGlobal("navigator", { platform: "linux" });
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_tree") return Promise.resolve([]);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "config_read") return Promise.resolve({});
      return Promise.resolve(undefined);
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("the h1 and the target bar sit inside the banner landmark", async () => {
    render(AppShell);
    await waitFor(() => expect(screen.getByTestId("status-text")).toBeTruthy());
    const banner = document.querySelector("header");
    expect(banner).not.toBeNull();
    const h1 = document.querySelector("h1");
    expect(h1).not.toBeNull();
    expect(banner!.contains(h1)).toBe(true);
    const targetBar = document.querySelector("[data-address-bar]");
    expect(targetBar).not.toBeNull();
    expect(banner!.contains(targetBar)).toBe(true);
  });
});

describe("AppShell startup MIB load (launch-beta/20)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedGetVersion.mockReset();
    mockedGetVersion.mockResolvedValue("9.9.9");
    vi.stubGlobal("navigator", { platform: "linux" });
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_tree") return Promise.resolve([]);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "config_read")
        return Promise.resolve({ mib: { directories: [], files: [] } });
      if (cmd === "mib_load")
        return Promise.resolve({ nodeCount: 4200, fallbackMibs: [], filesParsed: 60, filesCached: 0 });
      return Promise.resolve(undefined);
    });
    S.statusText = "Ready";
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    S.statusText = "Ready";
  });

  it("calls mib_load even when the configured MIB sources are empty", async () => {
    render(AppShell);
    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith(
        "mib_load",
        expect.objectContaining({ directories: [], files: [] }),
      ),
    );
  });

  it("shows the bundled-MIBs copy (not '0 directory(ies)') while loading", async () => {
    let statusAtLoad = "";
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_tree") return Promise.resolve([]);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      if (cmd === "config_read")
        return Promise.resolve({ mib: { directories: [], files: [] } });
      if (cmd === "mib_load") {
        statusAtLoad = S.statusText;
        return Promise.resolve({ nodeCount: 4200, fallbackMibs: [], filesParsed: 60, filesCached: 0 });
      }
      return Promise.resolve(undefined);
    });

    render(AppShell);
    await waitFor(() => expect(S.statusText).toBe("Ready"));
    expect(statusAtLoad).toBe("Loading bundled MIBs...");
  });
});
