import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ConnectionModal from "./ConnectionModal.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedInvoke.mockResolvedValue(undefined);
  S.connectionPanelOpen = true;
  S.saveCredentials = true;
});

afterEach(() => {
  S.connectionPanelOpen = false;
  S.saveCredentials = true;
});

describe("ConnectionModal host and port fields (launch-beta/31)", () => {
  it("edits update the shared target config the toolbar also reads", async () => {
    S.targetConfig.host = "192.0.2.1";
    S.targetConfig.port = 161;
    const view = render(ConnectionModal);

    await fireEvent.input(view.getByTestId("connection-host-input"), {
      target: { value: "10.0.0.5" },
    });
    expect(S.targetConfig.host).toBe("10.0.0.5");

    await fireEvent.input(view.getByTestId("connection-port-input"), {
      target: { value: "1610" },
    });
    expect(S.targetConfig.port).toBe(1610);
  });

  it("invalid port keystrokes never clobber the configured port", async () => {
    S.targetConfig.port = 11611;
    const view = render(ConnectionModal);

    for (const raw of ["", "0", "70000", "abc"]) {
      await fireEvent.input(view.getByTestId("connection-port-input"), {
        target: { value: raw },
      });
    }
    expect(S.targetConfig.port).toBe(11611);
  });
});

describe("ConnectionModal credential persistence note", () => {
  it("states honestly that credentials are saved to the config file (toggle on)", () => {
    const view = render(ConnectionModal);

    expect(view.getByTestId("credentials-note").textContent).toContain(
      "saved to the local config file",
    );
    expect((view.getByTestId("save-credentials-toggle") as HTMLInputElement).checked).toBe(true);
  });

  it("states that credentials will not be saved when the toggle is off", () => {
    S.saveCredentials = false;
    const view = render(ConnectionModal);

    expect(view.getByTestId("credentials-note").textContent).toContain(
      "will not be saved and must be re-entered on each launch",
    );
  });

  it("toggling off persists the opt-out via config_write", async () => {
    const view = render(ConnectionModal);
    const toggle = view.getByTestId("save-credentials-toggle") as HTMLInputElement;

    await fireEvent.click(toggle);

    expect(S.saveCredentials).toBe(false);
    expect(mockedInvoke).toHaveBeenCalledWith("config_write", {
      path: "ui.save_credentials",
      value: false,
    });
    expect(view.getByTestId("credentials-note").textContent).toContain(
      "will not be saved and must be re-entered on each launch",
    );
  });

  it("toggling back on persists save_credentials = true", async () => {
    S.saveCredentials = false;
    const view = render(ConnectionModal);
    const toggle = view.getByTestId("save-credentials-toggle") as HTMLInputElement;

    await fireEvent.click(toggle);

    expect(S.saveCredentials).toBe(true);
    expect(mockedInvoke).toHaveBeenCalledWith("config_write", {
      path: "ui.save_credentials",
      value: true,
    });
    expect(view.getByTestId("credentials-note").textContent).toContain(
      "saved to the local config file",
    );
  });
});

// axe (launch-beta/13): the app heading is an h1; the modal titled with h3
// skipped a level (heading-order, both version states). HexViewModal already
// titles itself with an h2 — the modal heading level is h2 app-wide.
describe("ConnectionModal heading order (launch-beta/13)", () => {
  it("titles the dialog with an h2", () => {
    const view = render(ConnectionModal);
    const title = view.container.querySelector("#connection-dialog-title");
    expect(title?.tagName).toBe("H2");
  });
});
