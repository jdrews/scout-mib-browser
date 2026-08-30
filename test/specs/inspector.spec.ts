import { expandTo, go, oidInputValue, selectTreeNode, typeOid, waitForAppReady, waitForStatus } from "../support/helpers";

describe("Inspector pane", () => {
  before(async () => {
    // The app process persists across spec files (one shared session). Reload
    // for a clean state: in this WebKitGTK/Xvfb environment each navigation
    // gives the webview fresh storage, so the pane starts from its defaults
    // (open, 240px tall, nothing selected). NOTE: localStorage written by the
    // app's main world is not visible to the driver's execute context (see
    // the theme note in menus-settings.spec.ts), so persisted values are not
    // assertable from e2e here.
    await browser.url("http://localhost:5173");
    await waitForAppReady();
  });

  it("is open by default with its title bar", async () => {
    const pane = await $("[data-testid='inspector-pane']");
    await expect(pane).toBeExisting();

    const toggle = await $("[data-testid='inspector-toggle']");
    expect(await toggle.getAttribute("aria-expanded")).toBe("true");
    expect((await toggle.getText()) ?? "").toContain("Inspector");
    await expect(await $("[data-testid='inspector-body']")).toBeExisting();
  });

  it("shows MIB details for a tree-selected node", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");

    const name = await $("[data-testid='inspector-name']");
    await expect(name).toBeExisting();
    expect(await name.getText()).toBe("sysDescr");

    const oid = (await (await $("[data-testid='inspector-oid']")).getText()) ?? "";
    expect(oid).toContain("1.3.6.1.2.1.1.1");

    // SYNTAX DisplayString reports its base type.
    expect((await (await $("[data-testid='inspector-type']")).getText()) ?? "").toBe("OctetString");

    const desc = (await (await $("[data-testid='inspector-description']")).getText()) ?? "";
    expect(desc).toContain("A textual description of the entity.");

    // Attribute rows carry MAX-ACCESS / STATUS / SIZE constraint.
    const attrs = (await (await $("[data-testid='inspector-attrs']")).getText()) ?? "";
    expect(attrs).toContain("Access");
    expect(attrs).toContain("read-only");
    expect(attrs).toContain("Status");
    expect(attrs).toContain("current");
    expect(attrs).toContain("SIZE (0..255)");
  });

  it("shows table metadata for a TABLE node", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "interfaces"]);
    await selectTreeNode("ifTable");

    const section = await $("[data-testid='inspector-table-section']");
    await expect(section).toBeExisting();
    const text = (await section.getText()) ?? "";
    // INDEX { ifIndex } is surfaced as the row key.
    expect(text).toContain("ifIndex");
  });

  it("shows enum values as a value → name list", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "synthTableMib", "synthObjects"]);
    await selectTreeNode("synthState");

    const section = await $("[data-testid='inspector-enums']");
    await expect(section).toBeExisting();

    // Header carries the count; each entry is a row pairing value with name.
    const header = (await section.getText()) ?? "";
    expect(header).toContain("Values (6)");
    expect(header).toContain("unknown");
    expect(header).toContain("draining");

    // Rows are list items, not chip badges.
    const rowCount = await browser.execute(() => {
      const el = document.querySelector("[data-testid='inspector-enums']");
      return el ? el.querySelectorAll("li").length : -1;
    });
    expect(rowCount).toBe(6);
  });

  it("updates when an autocomplete item is picked", async () => {
    await typeOid("sysuptime");
    await browser.pause(600); // past the 150 ms debounce
    const list = await $("[data-testid='autocomplete-list']");
    await expect(list).toBeExisting();

    const rows = await $$("[data-testid='autocomplete-list'] > div");
    let picked = false;
    for (const r of rows) {
      if (((await r.getText()) ?? "").includes("sysUpTime")) {
        await r.click();
        picked = true;
        break;
      }
    }
    expect(picked).toBe(true);

    const name = await $("[data-testid='inspector-name']");
    await expect(name).toBeExisting();
    expect(await name.getText()).toBe("sysUpTime");
    // The address bar shows the picked OID too.
    expect(await oidInputValue()).toContain("1.3.6.1.2.1.1.3");
  });

  it("collapses to just the title bar and reopens", async () => {
    const toggle = await $("[data-testid='inspector-toggle']");
    await toggle.click();
    expect(await (await $("[data-testid='inspector-toggle']")).getAttribute("aria-expanded")).toBe("false");
    // Body and resize handle are gone; only the title bar remains.
    await expect(await $("[data-testid='inspector-body']")).not.toBeExisting();
    await expect(await $("[data-testid='inspector-resize']")).not.toBeExisting();
    expect((await (await $("[data-testid='inspector-toggle']")).getText()) ?? "").toContain("Inspector");

    // Reopen: the body and resize handle come back.
    await (await $("[data-testid='inspector-toggle']")).click();
    expect(await (await $("[data-testid='inspector-toggle']")).getAttribute("aria-expanded")).toBe("true");
    await expect(await $("[data-testid='inspector-body']")).toBeExisting();
  });

  it("shows the live value when a result row is selected", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");
    await go();
    await waitForStatus(/Get complete: \d+ binding\(s\)/, 30000);

    const row = await $("[data-testid='result-row']");
    await expect(row).toBeExisting();
    await row.click();

    // The instance OID (…1.1.0) resolves back to the base node sysDescr.
    const name = await $("[data-testid='inspector-name']");
    await expect(name).toBeExisting();
    expect(await name.getText()).toBe("sysDescr");

    const live = await $("[data-testid='inspector-live-value']");
    await expect(live).toBeExisting();
    const text = (await live.getText()) ?? "";
    expect(text).toContain("Linux cray");
    expect(text).toContain("OCTET STRING");

    // The selected row is highlighted in the results list.
    expect(await row.getAttribute("class")).toContain("inspector-selected");
  });

  it("resizes by dragging its top handle", async () => {
    const pane = await $("[data-testid='inspector-pane']");
    const before = await pane.getSize();

    const handle = await $("[data-testid='inspector-resize']");
    await expect(handle).toBeExisting();
    expect(await handle.getAttribute("role")).toBe("separator");
    expect(await handle.getAttribute("aria-orientation")).toBe("horizontal");

    // The embedded driver's pointer-action support is broken (browser.actions
    // throws "action2.toJSON is not a function"), so the drag is emulated with
    // synthetic DOM MouseEvents — the same approach the helpers use for focus
    // movement. Dragging the top edge UP 60px grows the pane by 60px.
    const dragged = await browser.execute(() => {
      const el = document.querySelector("[data-testid='inspector-resize']");
      if (!el) return "no-handle";
      const r = el.getBoundingClientRect();
      el.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, clientX: r.x + 10, clientY: r.y + 2, button: 0 }));
      document.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: r.x + 10, clientY: r.y - 58, button: 0 }));
      document.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: r.x + 10, clientY: r.y - 58, button: 0 }));
      return "ok";
    });
    expect(dragged).toBe("ok");

    const after = await pane.getSize();
    expect(after.height).toBeGreaterThan(before.height + 40); // allow driver rounding
  });

  it("resizes with ArrowUp when the handle is focused", async () => {
    const pane = await $("[data-testid='inspector-pane']");
    const before = await pane.getSize();

    // The driver can't be relied on to Tab focus there (see the ux-03 note),
    // so focus the handle directly; ArrowUp/ArrowDown are real W3C key events.
    const focused = await browser.execute(() => {
      const el = document.querySelector("[data-testid='inspector-resize']");
      if (!el) return false;
      el.focus();
      return document.activeElement === el;
    });
    expect(focused).toBe(true);

    await browser.keys(["ArrowUp"]);
    await browser.keys(["ArrowUp"]);
    const after = await pane.getSize();
    expect(after.height).toBeGreaterThan(before.height + 10); // 2 × 16px step, allow rounding
  });
});
