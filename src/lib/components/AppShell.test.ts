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
