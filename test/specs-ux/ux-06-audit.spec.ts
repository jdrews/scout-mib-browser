import { AGENT_HOST, AGENT_PORT, expandTo, go, selectTreeNode, setOperation, typeOid, waitForStatus } from "../support/helpers";
import { accessibleNameAudit, collectUserStrings, freshWindow, injectAxe, runAxe, setTheme, shot, tabOrderWalk, writeJson, type AxeViolation, type NameAuditRow, type TabStop } from "../support/ux";

// A6 — Accessibility / DOM audit per state (axe-core devDependency + manual DOM
// checks) and A7 — terminology string collection. One shared app instance;
// states are scripted in order. axe must be re-injected after every reload.
// Written to docs/ux/<date>/axe.json, dom-audit.json, strings.json.
const UNKNOWN_INSTANCE_OID = "1.3.6.1.2.1.31.1.1.1.19.9999";

describe("UX A6/A7 — axe + DOM audit + terminology collection", function () {
  this.timeout(600000);

  const axeByState: Record<string, AxeViolation[]> = {};
  const nameAudit: Record<string, NameAuditRow[]> = {};
  const strings: Record<string, Record<string, string[]>> = {};
  let tabOrder: TabStop[] = [];

  async function axeState(name: string) {
    await injectAxe();
    axeByState[name] = await runAxe();
    console.log(`[ux][axe] ${name}: ${axeByState[name].length} violation rule(s)`);
  }

  before(async () => {
    await freshWindow();
    await setTheme("dark"); // earlier specs may have toggled the theme
  });

  it("audits each key state", async () => {
    // ── State: launch ready (empty results) ──────────────────────────────────
    tabOrder = await tabOrderWalk(40);
    nameAudit["launch-ready"] = await accessibleNameAudit();
    strings["launch-ready"] = await collectUserStrings();
    await axeState("launch-ready");

    // ── State: tree selected + autocomplete open ─────────────────────────────
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");
    await typeOid("sysdescr");
    await (await $("[data-testid='autocomplete-list']")).waitForExist({ timeout: 5000 });
    nameAudit["tree-selected-autocomplete"] = await accessibleNameAudit();
    strings["tree-selected-autocomplete"] = await collectUserStrings();
    await axeState("tree-selected-autocomplete");
    await shot("audit-tree-autocomplete");
    await (await $("[data-testid='oid-input']")).click();
    await browser.keys(["Escape"]);

    // ── State: results list view with controls visible ───────────────────────
    await go("get");
    await waitForStatus(/^Get complete: \d+ binding\(s\)$/, 30000);
    nameAudit["results-list"] = await accessibleNameAudit();
    strings["results-list"] = await collectUserStrings();
    await axeState("results-list");

    // ── State: grid view (ifTable) ───────────────────────────────────────────
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "interfaces"]);
    await selectTreeNode("ifTable");
    await setOperation("getTable");
    await go();
    await waitForStatus(/^Table complete: \d+ row\(s\), \d+ column\(s\)$/, 60000);
    nameAudit["grid-view"] = await accessibleNameAudit();
    strings["grid-view"] = await collectUserStrings();
    await axeState("grid-view");

    // ── State: warnings banner + partial badge ───────────────────────────────
    await freshWindow();
    await setTheme("dark");
    await typeOid(UNKNOWN_INSTANCE_OID);
    await go("get");
    await waitForStatus(/^Get complete: \d+ binding\(s\)$/, 30000);
    strings["warnings-partial"] = await collectUserStrings();
    await axeState("warnings-partial");

    // ── State: connection modal v2c / v3 ─────────────────────────────────────
    await (await $("[data-testid='conn-gear']")).click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000 });
    nameAudit["conn-modal-v2c"] = await accessibleNameAudit();
    strings["conn-modal-v2c"] = await collectUserStrings();
    await axeState("conn-modal-v2c");

    await browser.execute(() => {
      const panel = document.querySelector("[data-connection-panel]");
      const btn = Array.from(panel!.querySelectorAll("button")).find((b) => (b.textContent || "").trim() === "SNMP V3");
      btn?.click();
    });
    await browser.pause(200);
    nameAudit["conn-modal-v3"] = await accessibleNameAudit();
    strings["conn-modal-v3"] = await collectUserStrings();
    await axeState("conn-modal-v3");

    const closeBtn = (await $$("[data-connection-panel] button"))[0];
    await closeBtn.click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000, reverse: true });

    // ── State: Manage MIBs dialog ────────────────────────────────────────────
    await (await $("[data-testid='menu-file']")).click();
    await (await $("[data-testid='menu-manage-mibs']")).click();
    await (await $("[data-testid='manage-mibs-dialog']")).waitForExist({ timeout: 5000 });
    nameAudit["manage-mibs"] = await accessibleNameAudit();
    strings["manage-mibs"] = await collectUserStrings();
    await axeState("manage-mibs");
    await (await $("[data-testid='manage-mibs-close']")).click();

    // ── State: System Log pane open ──────────────────────────────────────────
    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-system-log']")).click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ timeout: 5000 });
    strings["syslog-open"] = await collectUserStrings();
    await axeState("syslog-open");

    writeJson("axe.json", { tool: "axe-core 4.10.2 (npm devDependency)", states: axeByState });
    writeJson("dom-audit.json", {
      environment: `Xvfb, embedded WebKitGTK driver, agent ${AGENT_HOST}:${AGENT_PORT}`,
      tabOrderFromBody: tabOrder,
      accessibleNames: nameAudit,
    });
    writeJson("strings.json", strings);
  });
});
