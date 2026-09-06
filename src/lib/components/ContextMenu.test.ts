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
