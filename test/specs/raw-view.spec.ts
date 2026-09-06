import {
  CHAIN_TO_INTERFACES,
  expandTo,
  go,
  resultsBodyHasText,
  selectTreeNode,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

// Pinned linux-full-walk.snmprec (primary agent): ifTable has 2 rows x 22
// columns — 44 flat bindings when walked. Interface 2's ifPhysAddress is the
// 6-byte MAC 00:12:79:62:f9:40; interface 1's is an empty OctetString.
const IF_TABLE_FLAT_BINDINGS = 44;

/** Clicks the flat result row whose text contains `text`. */
async function clickRowContaining(text: string): Promise<void> {
  const found = await browser.execute((t: string) => {
    const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
    const row = rows.find((r) => (r.textContent ?? "").includes(t));
    if (!row) return false;
    row.setAttribute("data-e2e-target", "1");
    return true;
  }, text);
  if (!found) throw new Error(`result row containing "${text}" not found`);
  await (await $("[data-testid='result-row'][data-e2e-target='1']")).click();
  await browser.execute(() => {
    document.querySelectorAll("[data-e2e-target]").forEach((n) => n.removeAttribute("data-e2e-target"));
  });
}

describe("Raw view (hex + text) of OID results", () => {
  before(async () => {
    await waitForAppReady();
    await expandTo(CHAIN_TO_INTERFACES);
    await selectTreeNode("ifTable");
    await go("walk");
    await waitForStatus(new RegExp(`^walk complete: ${IF_TABLE_FLAT_BINDINGS} binding\\(s\\)$`), 60000);
  });

  // The Raw toggle is component state — leave it off so later specs see the
  // default presentation.
  after(async () => {
    const toggle = await $("[data-testid='raw-toggle']");
    if (await toggle.isExisting() && ((await toggle.getAttribute("class")) ?? "").includes("btn-primary")) {
      await toggle.click();
      await browser.pause(200);
    }
  });

  it("recognizes MAC addresses in OctetString values", async () => {
    // ifPhysAddress.2 (6 bytes) renders as a colon-separated MAC, not hex.
    expect(await resultsBodyHasText("00:12:79:62:f9:40")).toBe(true);
  });

  it("clicking a byte value shows its hex dump in the inspector", async () => {
    await clickRowContaining("00:12:79:62:f9:40");
    const dump = await $("[data-testid='inspector-hexdump']");
    await expect(dump).toBeExisting();
    const text = (await dump.getText()) ?? "";
    expect(text).toContain("recognized as mac address");
    expect(text).toContain("00 12 79 62 f9 40");
    // The ascii column renders non-printable bytes as dots.
    expect(text).toContain("..yb.@");
  });

  it("Raw toggle renders byte values as a hex + ASCII dump", async () => {
    await (await $("[data-testid='raw-toggle']")).click();
    await browser.pause(300);

    // The MAC row carries its recognized form plus the wire bytes: an
    // offset column, space-separated hex, and the interpretation line.
    expect(await resultsBodyHasText("MAC address: 00:12:79:62:f9:40")).toBe(true);
    expect(await resultsBodyHasText("00 12 79 62 f9 40")).toBe(true);
    expect(await resultsBodyHasText("0000")).toBe(true);

    // Text values keep a readable ascii column (ifDescr.2 = "eth0").
    expect(await resultsBodyHasText("eth0")).toBe(true);
  });

  it("Raw toggle shows scalars with their wire encoding", async () => {
    // ifSpeed.2 is INTEGER 1500 on the recording.
    expect(await resultsBodyHasText("0x000005dc (1500)")).toBe(true);
  });

  it("toggling Raw off restores the readable presentation", async () => {
    await (await $("[data-testid='raw-toggle']")).click();
    await browser.pause(300);

    expect(await resultsBodyHasText("00:12:79:62:f9:40")).toBe(true);
    // The dump-only artifacts are gone.
    expect(await resultsBodyHasText("MAC address: 00:12:79:62:f9:40")).toBe(false);
    expect(await resultsBodyHasText("00 12 79 62 f9 40")).toBe(false);
    expect(await resultsBodyHasText("0x000005dc (1500)")).toBe(false);
  });
});
