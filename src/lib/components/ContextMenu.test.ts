import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import ContextMenu from "./ContextMenu.svelte";
import HexViewModal from "./HexViewModal.svelte";
import { S } from "$lib/stores.svelte";

afterEach(() => {
  S.contextMenuTarget = null;
  S.hexViewTarget = null;
});

describe("ContextMenu", () => {
  it("opens the hex view modal when Hex View is clicked for a value target", async () => {
    S.contextMenuTarget = {
      kind: "value",
      oid: "1.3.6.1.2.1.1.1.0",
      displayName: "sysDescr.0",
      value: { OctetString: [0xde, 0xad] },
      writable: null,
      x: 10,
      y: 10,
    };
    const menu = render(ContextMenu);
    const modal = render(HexViewModal);

    await fireEvent.click(menu.getByTestId("ctx-hex-view"));

    await waitFor(() => expect(S.hexViewTarget).toEqual({
      oid: "1.3.6.1.2.1.1.1.0",
      displayName: "sysDescr.0",
      value: { OctetString: [0xde, 0xad] },
    }));
    expect(modal.queryByTestId("hex-view-modal")).not.toBeNull();
    // The menu itself closed after the action.
    expect(menu.queryByTestId("ctx-hex-view")).toBeNull();
  });

  it("offers Copy OID / Copy Name for node targets and no Hex View", () => {
    S.contextMenuTarget = {
      kind: "node",
      node: { oid: "1.3.6.1", name: "mib-2", mibName: "SNMPv2-MIB" },
      x: 0,
      y: 0,
    };
    const menu = render(ContextMenu);

    expect(menu.getByTestId("ctx-copy-oid")).not.toBeNull();
    expect(menu.getByTestId("ctx-copy-name")).not.toBeNull();
    expect(menu.queryByTestId("ctx-hex-view")).toBeNull();
  });

  it("renders nothing when no target is set", () => {
    const menu = render(ContextMenu);
    expect(menu.queryByTestId("ctx-copy-oid")).toBeNull();
    expect(menu.queryByTestId("ctx-hex-view")).toBeNull();
  });
});

describe("ContextMenu row targets (results rows)", () => {
  afterEach(() => {
    S.contextMenuTarget = null;
    S.revealInTree = null;
    S.setValueTarget = null;
    S.hexViewTarget = null;
  });

  it("offers Reveal + Copy OID + Copy Name, and reveal requests the base node", async () => {
    S.contextMenuTarget = {
      kind: "row",
      oid: "1.3.6.1.2.1.2.1.0",
      displayName: "ifNumber.0",
      baseOid: "1.3.6.1.2.1.2.1",
      writable: null,
      x: 5,
      y: 5,
    };
    const menu = render(ContextMenu);

    expect(menu.getByTestId("ctx-reveal")).not.toBeNull();
    expect(menu.getByTestId("ctx-copy-oid")).not.toBeNull();
    expect(menu.getByTestId("ctx-copy-name")).not.toBeNull();
    expect(menu.queryByTestId("ctx-set-value")).toBeNull();
    expect(menu.queryByTestId("ctx-hex-view")).toBeNull();

    await fireEvent.click(menu.getByTestId("ctx-reveal"));
    expect(S.revealInTree?.oid).toBe("1.3.6.1.2.1.2.1");
    expect(S.revealInTree?.seq).toBeGreaterThan(0);
    // The menu closed after the action.
    expect(menu.queryByTestId("ctx-reveal")).toBeNull();
  });

  it("hides Reveal when the OID is in no loaded MIB", () => {
    S.contextMenuTarget = {
      kind: "row",
      oid: "1.3.99.1.0",
      displayName: "1.3.99.1.0",
      baseOid: null,
      writable: null,
      x: 0,
      y: 0,
    };
    const menu = render(ContextMenu);
    expect(menu.queryByTestId("ctx-reveal")).toBeNull();
    expect(menu.getByTestId("ctx-copy-oid")).not.toBeNull();
  });

  it("offers the value actions for a row carrying a writable byte value", async () => {
    S.contextMenuTarget = {
      kind: "row",
      oid: "1.3.6.1.2.1.1.1.0",
      displayName: "sysDescr.0",
      baseOid: "1.3.6.1.2.1.1.1",
      value: { OctetString: [0x72, 0x6f] },
      writable: true,
      x: 0,
      y: 0,
    };
    const menu = render(ContextMenu);
    expect(menu.getByTestId("ctx-set-value")).not.toBeNull();
    expect(menu.getByTestId("ctx-hex-view")).not.toBeNull();

    await fireEvent.click(menu.getByTestId("ctx-hex-view"));
    expect(S.hexViewTarget?.oid).toBe("1.3.6.1.2.1.1.1.0");
  });

  it("row Set value opens the dialog with the row's live value", async () => {
    S.contextMenuTarget = {
      kind: "row",
      oid: "1.3.6.1.2.1.1.1.0",
      displayName: "sysDescr.0",
      baseOid: "1.3.6.1.2.1.1.1",
      value: { OctetString: [...new TextEncoder().encode("ro")] },
      writable: true,
      x: 0,
      y: 0,
    };
    const menu = render(ContextMenu);
    await fireEvent.click(menu.getByTestId("ctx-set-value"));

    await waitFor(() => expect(S.setValueTarget).not.toBeNull());
    expect(S.setValueTarget?.oid).toBe("1.3.6.1.2.1.1.1.0");
    expect(S.setValueTarget?.currentValue).toBe('"ro"'); // valueDisplay quotes OctetStrings
  });
});
