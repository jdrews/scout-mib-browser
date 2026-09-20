import {
  AGENT_PORT,
  CHAIN_TO_MIB2,
  SET_AGENT_PORT,
  expandTo,
  go,
  restoreTargetPort,
  selectTreeNode,
  setTargetPort,
  typeOid,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

// Set-capable fixture: test/mibs/SYNTH-SET-MIB served by
// test/snmprec/set-capable.snmprec. snmpsim persists successful Sets for the
// lifetime of the agent process, so the tests run in a deliberate order and
// assert against the evolving state (initial: scout / 0 / 42 / 7 / note).
const NAME_OID = "1.3.6.1.2.1.15433.1.0"; // DisplayString read-write, DEFVAL scout
const MODE_OID = "1.3.6.1.2.1.15433.2.0"; // enum off(0)/on(1)/auto(2) read-write
const GUARDED_OID = "1.3.6.1.2.1.15433.3.0"; // Integer32 read-write; agent rejects 999
const READONLY_OID = "1.3.6.1.2.1.15433.4.0"; // Integer32 read-only
const NOTE_OID = "1.3.6.1.2.1.15433.5.0"; // OCTET STRING read-only

// The driver can't select by text, so row-level assertions dispatch through
// page-context helpers (self-contained: no references to this file's scope).

/** Number of flat result rows currently rendered. */
async function rowCount(): Promise<number> {
  return await browser.execute(
    () => document.querySelectorAll("[data-testid='result-row']").length
  );
}

/** Normalized text of the value cell (second child) of the first row whose
 *  text contains `needle`; null when no row matches. */
async function rowValue(needle: string): Promise<string | null> {
  return await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
    const row = rows.find((r) => norm(r.textContent).includes(t));
    if (!row) return null;
    const cell = row.children[1] as HTMLElement | undefined;
    return cell ? norm(cell.textContent) : null;
  }, needle);
}

/** True when the first row containing `needle` renders a Set pencil. */
async function rowHasPencil(needle: string): Promise<boolean> {
  return await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
    const row = rows.find((r) => norm(r.textContent).includes(t));
    return !!row && row.querySelector("[data-testid='set-pencil']") !== null;
  }, needle);
}

/** Clicks the Set pencil on the first row containing `needle`. */
async function clickPencilOnRow(needle: string): Promise<void> {
  const found = await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
    for (const r of rows) {
      r.querySelector("[data-testid='set-pencil']")?.removeAttribute("data-e2e-target");
    }
    const row = rows.find((r) => norm(r.textContent).includes(t));
    const pencil = row?.querySelector("[data-testid='set-pencil']");
    if (!pencil) return false;
    pencil.setAttribute("data-e2e-target", "1");
    return true;
  }, needle);
  if (!found) throw new Error(`Set pencil not found on row "${needle}"`);
  await (await $("[data-testid='set-pencil'][data-e2e-target='1']")).click();
}

/** Clicks the first result row containing `needle` (selects it in the inspector). */
async function clickRow(needle: string): Promise<void> {
  const found = await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
    // Clear stale markers from previous clicks — querySelector returns the
    // first match in document order, so a leftover attribute would re-click
    // the earlier row.
    for (const r of rows) r.removeAttribute("data-e2e-target");
    const row = rows.find((r) => norm(r.textContent).includes(t));
    if (!row) return false;
    row.setAttribute("data-e2e-target", "1");
    return true;
  }, needle);
  if (!found) throw new Error(`result row "${needle}" not found`);
  await (await $("[data-testid='result-row'][data-e2e-target='1']")).click();
}

/** Dispatches a contextmenu event on the value cell of the first row
 *  containing `needle`. */
async function rightClickRowValue(needle: string): Promise<boolean> {
  return await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
    const row = rows.find((r) => norm(r.textContent).includes(t));
    const cell = row?.children[1] as HTMLElement | undefined;
    if (!cell) return false;
    const rect = cell.getBoundingClientRect();
    cell.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + 24,
        clientY: rect.top + 6,
      })
    );
    return true;
  }, needle);
}

/** Dismisses the context menu (document click listener). */
async function closeContextMenu(): Promise<void> {
  await browser.execute(() => {
    document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
}

/** Sets a <select> by value and fires the change event Svelte listens for. */
async function selectValue(selector: string, value: string): Promise<void> {
  await browser.execute((sel: string, v: string) => {
    const s = document.querySelector(sel) as HTMLSelectElement;
    s.value = v;
    s.dispatchEvent(new Event("change", { bubbles: true }));
  }, selector, value);
}

async function dialogClosed(): Promise<void> {
  await browser.waitUntil(
    async () => !(await $("[data-testid='set-value-dialog']").isExisting()),
    { timeout: 10000, timeoutMsg: "Set dialog did not close" }
  );
}

describe("SNMP Set (set-capable agent)", () => {
  before(async () => {
    await waitForAppReady();
    await setTargetPort(SET_AGENT_PORT);
  });

  after(async () => {
    await restoreTargetPort();
  });

  it("walk + address-bar Set updates the result row in place", async () => {
    await typeOid("1.3.6.1.2.1.15433");
    await go("walk");
    const status = await waitForStatus(/^walk complete: \d+ binding\(s\)$/);
    expect(status).toBe("walk complete: 5 binding(s)");

    // Initial recording state.
    expect(await rowValue("synthSetName.0")).toBe('"scout"');

    await expandTo([...CHAIN_TO_MIB2, "synthSetMib"]);
    await selectTreeNode("synthSetName");
    await go("set");

    const dialog = await $("[data-testid='set-value-dialog']");
    await expect(dialog).toBeExisting();
    // Address-bar entry carries no live value → prefill from DEFVAL.
    expect(await (await $("[data-testid='set-octet-input']")).getValue()).toBe("scout");
    expect((await (await $("[data-testid='set-dialog-title']")).getText()) ?? "").toContain(
      "synthSetName"
    );
    // The scalar .0 fixup is applied to the OID shown in the dialog.
    expect((await (await $("[data-testid='set-dialog-oid']")).getText()) ?? "").toBe(NAME_OID);

    await (await $("[data-testid='set-octet-input']")).setValue("updated");
    await (await $("[data-testid='set-submit']")).click();

    const done = await waitForStatus(/^Set complete: /);
    expect(done).toBe('Set complete: synthSetName = "updated"');

    // The dialog closes and the row updates in place; siblings are preserved.
    await dialogClosed();
    expect(await rowCount()).toBe(5);
    expect(await rowValue("synthSetName.0")).toBe('"updated"');
    expect(await rowValue("synthSetGuarded.0")).toBe("42");
  });

  it("enum editor sends the numeric value of the chosen name", async () => {
    await selectTreeNode("synthSetMode");
    await go("set");

    const sel = await $("[data-testid='set-enum-select']");
    await expect(sel).toBeExisting();
    const options = await browser.execute(() => {
      const s = document.querySelector("[data-testid='set-enum-select']") as HTMLSelectElement;
      return Array.from(s.options).map((o) => o.textContent ?? "");
    });
    expect(options).toContain("off (0)");
    expect(options).toContain("on (1)");
    expect(options).toContain("auto (2)");
    expect(options).toContain("Custom…");

    await selectValue("[data-testid='set-enum-select']", "2");
    await (await $("[data-testid='set-submit']")).click();

    const done = await waitForStatus(/^Set complete: /);
    expect(done).toBe("Set complete: synthSetMode = 2");
    expect(await rowValue("synthSetMode.0")).toBe("2");
  });

  it("Go → Set on a read-only OID is refused before any request", async () => {
    await selectTreeNode("synthSetReadOnly");
    await go("set");
    const status = await waitForStatus(/is read-only/);
    expect(status).toBe("synthSetReadOnly is read-only");
    expect(await (await $("[data-testid='set-value-dialog']")).isExisting()).toBe(false);
  });

  it("inspector offers Set on writable nodes only, prefilled from the live value", async () => {
    // Writable row → details resolve, then the button appears.
    await clickRow("synthSetName.0");
    await browser.waitUntil(
      async () => {
        const el = await $("[data-testid='inspector-oid']");
        if (!(await el.isExisting())) return false;
        return ((await el.getText()) ?? "").includes("15433.1");
      },
      { timeout: 10000, timeoutMsg: "inspector never showed synthSetName details" }
    );
    const btn = await $("[data-testid='inspector-set-btn']");
    await expect(btn).toBeExisting();
    await btn.click();

    const dialog = await $("[data-testid='set-value-dialog']");
    await expect(dialog).toBeExisting();
    // Prefill from the live (already Set) value, not DEFVAL.
    expect(await (await $("[data-testid='set-octet-input']")).getValue()).toBe("updated");
    await (await $("[data-testid='set-cancel']")).click();
    await dialogClosed();

    // Read-only row → the button never appears.
    await clickRow("synthSetReadOnly.0");
    await browser.waitUntil(
      async () => {
        const el = await $("[data-testid='inspector-oid']");
        if (!(await el.isExisting())) return false;
        return ((await el.getText()) ?? "").includes("15433.4");
      },
      { timeout: 10000, timeoutMsg: "inspector never showed synthSetReadOnly details" }
    );
    expect(await (await $("[data-testid='inspector-set-btn']")).isExisting()).toBe(false);
  });

  it("results pencil appears only on confirmed-writable rows and preserves siblings", async () => {
    // Writability resolves lazily per base OID — three of the five are read-write.
    await browser.waitUntil(
      async () => (await $$("[data-testid='set-pencil']")).length === 3,
      { timeout: 15000, timeoutMsg: "Set pencils did not appear on writable rows" }
    );
    await browser.pause(500); // let the read-only lookups settle before asserting absence
    expect(await rowHasPencil("synthSetReadOnly.0")).toBe(false);
    expect(await rowHasPencil("synthSetNote.0")).toBe(false);

    await clickPencilOnRow("synthSetGuarded.0");
    const input = await $("[data-testid='set-int-input']");
    await expect(input).toBeExisting();
    // Prefill from the live value.
    expect(await input.getValue()).toBe("42");

    await input.setValue("5");
    await (await $("[data-testid='set-submit']")).click();
    const done = await waitForStatus(/^Set complete: /);
    // Pencil entry names the target by the row's display name (instance OID form).
    expect(done).toBe("Set complete: synthSetGuarded.0 = 5");

    // The merge updates the row in place; sibling rows are untouched.
    expect(await rowCount()).toBe(5);
    expect(await rowValue("synthSetGuarded.0")).toBe("5");
    expect(await rowValue("synthSetName.0")).toBe('"updated"');
    expect(await rowValue("synthSetMode.0")).toBe("2");
  });

  it("agent rejection keeps the dialog open; retry succeeds", async () => {
    await clickPencilOnRow("synthSetGuarded.0");
    const input = await $("[data-testid='set-int-input']");
    await expect(input).toBeExisting();
    expect(await input.getValue()).toBe("5");

    // 999 is the value the mock agent rejects with wrongValue.
    await input.setValue("999");
    await (await $("[data-testid='set-submit']")).click();
    const failed = await waitForStatus(/^Set failed: /);
    expect(failed).toBe("Set failed: wrongValue");

    // The dialog stays open with the rejected value preserved for a retry.
    expect(await (await $("[data-testid='set-value-dialog']")).isExisting()).toBe(true);
    expect((await (await $("[data-testid='set-error']")).getText()) ?? "").toContain("wrongValue");
    expect(await input.getValue()).toBe("999");
    // The result row still shows the last successful value.
    expect(await rowValue("synthSetGuarded.0")).toBe("5");

    await input.setValue("6");
    await (await $("[data-testid='set-submit']")).click();
    const done = await waitForStatus(/^Set complete: /);
    expect(done).toBe("Set complete: synthSetGuarded.0 = 6");
    expect(await rowValue("synthSetGuarded.0")).toBe("6");
  });

  it("value-cell context menu: Set value… on writable, Hex View only on read-only bytes, none on read-only scalars", async () => {
    // Writable byte value → both items; Hex View still opens from the menu.
    expect(await rightClickRowValue("synthSetName.0")).toBe(true);
    await expect(await $("[data-testid='ctx-set-value']")).toBeExisting();
    await expect(await $("[data-testid='ctx-hex-view']")).toBeExisting();
    await (await $("[data-testid='ctx-hex-view']")).click();
    await expect(await $("[data-testid='hex-view-modal']")).toBeExisting();
    await (await $("[data-testid='hex-close-btn']")).click();
    await browser.waitUntil(
      async () => !(await $("[data-testid='hex-view-modal']").isExisting()),
      { timeout: 10000 }
    );

    // Read-only byte value → Hex View only.
    expect(await rightClickRowValue("synthSetNote.0")).toBe(true);
    expect(await (await $("[data-testid='ctx-hex-view']")).isExisting()).toBe(true);
    expect(await (await $("[data-testid='ctx-set-value']")).isExisting()).toBe(false);
    await closeContextMenu();

    // Read-only non-byte value → no app menu at all.
    expect(await rightClickRowValue("synthSetReadOnly.0")).toBe(true);
    expect(await (await $("[data-testid='ctx-set-value']")).isExisting()).toBe(false);
    expect(await (await $("[data-testid='ctx-hex-view']")).isExisting()).toBe(false);
  });
});

describe("SNMP Set (unknown access)", () => {
  before(async () => {
    await waitForAppReady();
    await setTargetPort(AGENT_PORT);
  });

  after(async () => {
    await restoreTargetPort();
  });

  it("out-of-tree rows: no pencil, context menu keeps Set value…, dialog falls back to the type picker", async () => {
    // ucdMem objects are in the recording but not in the loaded MIBs.
    await typeOid("1.3.6.1.4.1.2021.4");
    await go("walk");
    const status = await waitForStatus(/^walk complete: \d+ binding\(s\)$/);
    expect(status).toBe("walk complete: 12 binding(s)");

    // Unknown access is not confirmed-writable → no pencil affordance.
    await browser.pause(750);
    expect((await $$("[data-testid='set-pencil']")).length).toBe(0);

    // memErrorName (an OCTET STRING): right-click keeps Set value… available —
    // unknown is not the same as confirmed non-writable.
    expect(await rightClickRowValue("2021.4.2.0")).toBe(true);
    await expect(await $("[data-testid='ctx-set-value']")).toBeExisting();
    await (await $("[data-testid='ctx-set-value']")).click();

    const dialog = await $("[data-testid='set-value-dialog']");
    await expect(dialog).toBeExisting();
    // Longest-prefix resolution lands on the `enterprises` ObjectIdentifier
    // subtree — treated as unknown, so the type picker is offered.
    await expect(await $("[data-testid='set-type-picker']")).toBeExisting();
    const optionCount = await browser.execute(() => {
      const s = document.querySelector("[data-testid='set-type-picker']") as HTMLSelectElement;
      return Array.from(s.options).length;
    });
    expect(optionCount).toBe(8);

    await (await $("[data-testid='set-cancel']")).click();
    await dialogClosed();
  });

  it("address-bar Set on an unknown OID opens the type picker", async () => {
    await typeOid("2.999.999");
    await go("set");
    const dialog = await $("[data-testid='set-value-dialog']");
    await expect(dialog).toBeExisting();
    await expect(await $("[data-testid='set-type-picker']")).toBeExisting();
    // No MIB name → the title falls back to the raw OID.
    expect((await (await $("[data-testid='set-dialog-title']")).getText()) ?? "").toContain(
      "2.999.999"
    );
    await (await $("[data-testid='set-cancel']")).click();
    await dialogClosed();
  });
});
