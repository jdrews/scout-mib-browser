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
