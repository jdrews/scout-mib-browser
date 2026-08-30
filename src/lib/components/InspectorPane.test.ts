import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import InspectorPane from "./InspectorPane.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { MibNodeDetails } from "$lib/types";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

const sysDescrDetails: MibNodeDetails = {
  oid: "1.3.6.1.2.1.1.1",
  name: "sysDescr",
  mibName: "SNMPv2-MIB",
  syntaxType: "OctetString",
  description: "A textual description of the entity.",
  access: "read-only",
  status: "current",
  constraints: "SIZE (0..255)",
};

const enumDetails: MibNodeDetails = {
  oid: "1.3.6.1.2.1.15432.1.3",
  name: "synthState",
  mibName: "SYNTH-TABLE-MIB",
  syntaxType: "Integer32",
  description: "A synthetic status with a long enum list.",
  access: "read-only",
  status: "current",
  enums: [
    { label: "unknown", value: 0 },
    { label: "idle", value: 1 },
    { label: "active", value: 2 },
  ],
};

const ifTableDetails: MibNodeDetails = {
  oid: "1.3.6.1.2.1.2.2",
  name: "ifTable",
  mibName: "IF-MIB",
  syntaxType: "TABLE",
  isTable: true,
  description: "A table of interface information.",
  access: "not-accessible",
  status: "current",
  table: {
    tableOid: "1.3.6.1.2.1.2.2",
    name: "ifTable",
    rowEntryOids: ["1.3.6.1.2.1.2.2.1"],
    indexColumns: [
      { name: "ifIndex", oid: "1.3.6.1.2.1.2.2.1.1", implied: false, encoding: "Integer" },
    ],
    columnOids: ["1.3.6.1.2.1.2.2.1.1", "1.3.6.1.2.1.2.2.1.2"],
  },
};

function mockDetails(details: MibNodeDetails | null) {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "mib_node_details") return details;
    return undefined;
  });
}

beforeEach(() => {
  mockedInvoke.mockReset();
});

afterEach(() => {
  S.inspectorOid = null;
  S.inspectorValue = null;
  S.inspectorOpen = true;
  S.inspectorHeight = 240;
  S.treeVersion = 0;
});

describe("InspectorPane", () => {
  it("shows a placeholder when nothing is selected", () => {
    const view = render(InspectorPane);
    expect(view.getByTestId("inspector-placeholder")).toBeTruthy();
  });

  it("is open by default and shows the title bar", () => {
    const view = render(InspectorPane);
    const toggle = view.getByTestId("inspector-toggle");
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(toggle.textContent).toContain("Inspector");
    expect(view.getByTestId("inspector-body")).toBeTruthy();
  });

  it("fetches and renders details for the selected OID", async () => {
    mockDetails(sysDescrDetails);
    S.inspectorOid = "1.3.6.1.2.1.1.1";

    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-name").textContent).toBe("sysDescr"));

    expect(mockedInvoke).toHaveBeenCalledWith("mib_node_details", { oid: "1.3.6.1.2.1.1.1" });
    expect(view.getByTestId("inspector-oid").textContent).toBe("1.3.6.1.2.1.1.1");
    expect(view.getByTestId("inspector-type").textContent).toBe("OctetString");
    expect(view.getByTestId("inspector-description").textContent).toContain(
      "A textual description of the entity.",
    );
    // Attribute rows: label + value pairs from the details.
    const attrs = view.getByTestId("inspector-attrs");
    expect(attrs.textContent).toContain("Access");
    expect(attrs.textContent).toContain("read-only");
    expect(attrs.textContent).toContain("Status");
    expect(attrs.textContent).toContain("current");
    expect(attrs.textContent).toContain("SIZE (0..255)");
  });

  it("renders the table section for TABLE nodes", async () => {
    mockDetails(ifTableDetails);
    S.inspectorOid = "1.3.6.1.2.1.2.2";

    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-table-section")).toBeTruthy());
    const section = view.getByTestId("inspector-table-section");
    expect(section.textContent).toContain("2 column(s)");
    expect(section.textContent).toContain("ifIndex");
  });

  it("renders enum values as a value → name list, not chips", async () => {
    mockDetails(enumDetails);
    S.inspectorOid = "1.3.6.1.2.1.15432.1.3";

    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-enums")).toBeTruthy());
    const section = view.getByTestId("inspector-enums");

    // Header carries the count; each entry is a value/name row.
    expect(section.textContent).toContain("Values (3)");
    const rows = Array.from(section.querySelectorAll("li"));
    expect(rows.length).toBe(3);
    expect(rows[0].textContent).toContain("0");
    expect(rows[0].textContent).toContain("unknown");
    expect(rows[2].textContent).toContain("2");
    expect(rows[2].textContent).toContain("active");
    // No chip badges remain in the values section.
    expect(section.querySelectorAll(".badge").length).toBe(0);
  });

  it("shows a live value when one was captured from results", async () => {
    mockDetails(sysDescrDetails);
    S.inspectorOid = "1.3.6.1.2.1.1.1.0";
    S.inspectorValue = { text: "Linux cray 2.6.21.5-smp", typeLabel: "OCTET STRING" };

    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-live-value")).toBeTruthy());
    const live = view.getByTestId("inspector-live-value");
    expect(live.textContent).toContain("Linux cray 2.6.21.5-smp");
    expect(live.textContent).toContain("OCTET STRING");
  });

  it("reports OIDs that are not in the loaded MIBs", async () => {
    mockDetails(null);
    S.inspectorOid = "9.9.9.9";

    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-not-found")).toBeTruthy());
    expect(view.getByTestId("inspector-not-found").textContent).toContain("9.9.9.9");
  });

  it("collapses to just the title bar and back", async () => {
    mockDetails(sysDescrDetails);
    S.inspectorOid = "1.3.6.1.2.1.1.1";

    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-name")).toBeTruthy());

    const toggle = view.getByTestId("inspector-toggle");
    await fireEvent.click(toggle);
    expect(S.inspectorOpen).toBe(false);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    // Body and resize handle are gone; only the title bar remains.
    expect(view.queryByTestId("inspector-body")).toBeNull();
    expect(view.queryByTestId("inspector-resize")).toBeNull();
    expect(toggle.textContent).toContain("Inspector");

    await fireEvent.click(toggle);
    expect(S.inspectorOpen).toBe(true);
    await waitFor(() => expect(view.getByTestId("inspector-body")).toBeTruthy());
  });

  it("refetches details when the MIB set changes (treeVersion bump)", async () => {
    let calls = 0;
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd !== "mib_node_details") return undefined;
      calls++;
      // The node exists on the first fetch, then disappears (MIB unloaded).
      return calls === 1 ? sysDescrDetails : null;
    });

    S.inspectorOid = "1.3.6.1.2.1.1.1";
    const view = render(InspectorPane);
    await waitFor(() => expect(view.getByTestId("inspector-name").textContent).toBe("sysDescr"));

    S.treeVersion++;
    await waitFor(() => expect(view.getByTestId("inspector-not-found")).toBeTruthy());
    expect(calls).toBe(2);
  });

  it("resizes with ArrowUp/ArrowDown on the focusable handle", async () => {
    const view = render(InspectorPane);
    const handle = view.getByTestId("inspector-resize");
    expect(handle.getAttribute("role")).toBe("separator");
    expect(handle.getAttribute("tabindex")).toBe("0");

    // The handle is the pane's top edge: ArrowUp grows, ArrowDown shrinks.
    await fireEvent.keyDown(handle, { key: "ArrowUp" });
    expect(S.inspectorHeight).toBe(256); // 240 + step

    await fireEvent.keyDown(handle, { key: "ArrowDown" });
    await fireEvent.keyDown(handle, { key: "ArrowDown" });
    expect(S.inspectorHeight).toBe(224);

    // Clamped at the minimum height.
    for (let i = 0; i < 10; i++) await fireEvent.keyDown(handle, { key: "ArrowDown" });
    expect(S.inspectorHeight).toBe(120);
  });

  it("does not let a stale response clobber a newer selection", async () => {
    let firstResolve: (v: MibNodeDetails | null) => void = () => {};
    mockedInvoke.mockImplementation(async (cmd: string, args?: { oid?: string }) => {
      if (cmd !== "mib_node_details") return undefined;
      if (args?.oid === "1.1.1") {
        // First request resolves slowly with stale data.
        return new Promise((resolve) => (firstResolve = resolve));
      }
      return sysDescrDetails;
    });

    S.inspectorOid = "1.1.1";
    const view = render(InspectorPane);
    // Rapidly switch to a different OID before the first response lands.
    S.inspectorOid = "1.3.6.1.2.1.1.1";
    await waitFor(() => expect(view.getByTestId("inspector-name").textContent).toBe("sysDescr"));

    // Now let the stale response arrive — it must be ignored.
    firstResolve(null);
    await new Promise((r) => setTimeout(r, 20));
    expect(view.getByTestId("inspector-name").textContent).toBe("sysDescr");
  });
});
