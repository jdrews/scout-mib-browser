import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, screen, waitFor, cleanup } from "@testing-library/svelte";
import SetValueEditor from "./SetValueEditor.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { MibNodeDetails } from "$lib/types";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

const oidDetails: MibNodeDetails = {
  oid: "1.3.6.1.2.1.1.2",
  name: "sysObjectID",
  mibName: "SNMPv2-MIB",
  syntaxType: "ObjectIdentifier",
  description: "The vendor's OID identification.",
  access: "read-write",
  status: "current",
};

const target = {
  oid: "1.3.6.1.2.1.1.2",
  name: "sysObjectID",
  details: oidDetails,
};

function mockSearch(res: unknown) {
  mockedInvoke.mockImplementation((cmd: string) => {
    if (cmd === "mib_search") return Promise.resolve(res);
    return Promise.resolve(undefined);
  });
}

// The OID editor resolves MIB object names like the address bar: typing
// sysObjectID must surface "→ 1.3.6.1.2.1.1.2" after the debounce. The hint
// lands in a detached async callback (200 ms debounce + IPC), so it only
// reaches the DOM if it is $state — a plain let updates invisibly (Svelte 5
// non_reactive_update), which is what shipped.

describe("SetValueEditor OID name hint (Svelte 5 reactivity)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the resolved OID under a typed MIB object name", async () => {
    mockSearch([{ oid: "1.3.6.1.2.1.1.2", name: "sysObjectID", syntaxType: "ObjectIdentifier", mibName: "SNMPv2-MIB" }]);
    render(SetValueEditor, { props: { target, onClose: () => {} } });

    const input = screen.getByTestId("set-oid-input");
    await fireEvent.input(input, { target: { value: "sysObjectID" } });

    const hint = await waitFor(() => {
      const el = screen.queryByTestId("set-oid-hint");
      if (!el) throw new Error("OID hint never rendered");
      return el;
    }, { timeout: 2000, interval: 50 });
    expect(hint.textContent).toContain("1.3.6.1.2.1.1.2");
    expect(mockedInvoke).toHaveBeenCalledWith("mib_search", { query: "sysObjectID" });
  });

  it("shows no hint for a dotted OID (nothing to resolve)", async () => {
    mockSearch([]);
    render(SetValueEditor, { props: { target, onClose: () => {} } });

    const input = screen.getByTestId("set-oid-input");
    await fireEvent.input(input, { target: { value: "1.3.6.1.2.1.1.2" } });

    await new Promise((r) => setTimeout(r, 400));
    expect(screen.queryByTestId("set-oid-hint")).toBeNull();
    expect(mockedInvoke).not.toHaveBeenCalledWith("mib_search", expect.anything());
  });
});
