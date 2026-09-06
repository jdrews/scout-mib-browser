import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import HexViewModal from "./HexViewModal.svelte";
import { S } from "$lib/stores.svelte";

// 9 bytes: 4 unprintable + "Hello" — one dump row, exercises hex and ASCII.
const BYTES = [0xde, 0xad, 0xbe, 0xef, 0x48, 0x65, 0x6c, 0x6c, 0x6f];

// happy-dom exposes navigator.clipboard as a getter-only property.
const clipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function mockClipboard() {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  return writeText;
}

beforeEach(() => {
  S.hexViewTarget = null;
});

afterEach(() => {
  S.hexViewTarget = null;
  // Restore the original accessor, or drop the own property we shadowed.
  if (clipboardDescriptor) Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
  else delete (navigator as unknown as { clipboard?: unknown }).clipboard;
});

describe("HexViewModal", () => {
  it("renders nothing when no value is targeted", () => {
    const view = render(HexViewModal);
    expect(view.queryByTestId("hex-view-modal")).toBeNull();
  });

  it("shows the name, OID, and the full uncapped dump for a byte value", () => {
    S.hexViewTarget = { oid: "1.3.6.1.2.1.1.1.0", displayName: "sysDescr.0", value: { OctetString: BYTES } };
    const view = render(HexViewModal);

    expect(view.getByTestId("hex-view-name").textContent).toBe("sysDescr.0");
    const modal = view.getByTestId("hex-view-modal").textContent ?? "";
    expect(modal).toContain("1.3.6.1.2.1.1.1.0");
    expect(modal).toContain("9 bytes");
    const dump = view.getByTestId("hex-view-dump").textContent ?? "";
    expect(dump).toContain("de ad be ef 48 65 6c 6c 6f");
    expect(dump).toContain("Hello");
  });

  it("shows the BER type code for Raw values", () => {
    S.hexViewTarget = { oid: "1.0", displayName: "rawThing", value: { Raw: { type_code: 0x04, data: [1, 2] } } };
    const view = render(HexViewModal);

    expect(view.getByTestId("hex-view-modal").textContent).toContain("OCTET STRING");
  });

  it("labels recognized byte patterns", () => {
    S.hexViewTarget = { oid: "1.0", displayName: "macThing", value: { OctetString: [0x00, 0x12, 0x79, 0x62, 0xf9, 0x40] } };
    const view = render(HexViewModal);

    expect(view.getByTestId("hex-view-modal").textContent).toContain("MAC address: 00:12:79:62:f9:40");
  });

  it("copies space-separated hex pairs to the clipboard", async () => {
    const writeText = mockClipboard();
    S.hexViewTarget = { oid: "1.0", displayName: "x", value: { OctetString: [0xde, 0xad] } };
    const view = render(HexViewModal);

    await fireEvent.click(view.getByTestId("hex-copy-btn"));

    expect(writeText).toHaveBeenCalledWith("de ad");
    expect(S.statusText).toContain("Copied hex");
  });

  it("closes via the close button", async () => {
    S.hexViewTarget = { oid: "1.0", displayName: "x", value: { OctetString: [1] } };
    const view = render(HexViewModal);

    await fireEvent.click(view.getByTestId("hex-close-btn"));

    expect(S.hexViewTarget).toBeNull();
  });
});
