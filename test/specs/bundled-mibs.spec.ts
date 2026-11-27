import {
  CHAIN_TO_SYSTEM,
  expandTo,
  go,
  selectTreeNode,
  typeOid,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

// The harness seeds an empty [mib] config only under E2E_EMPTY_MIBS=1 (the
// dedicated CI run); the flag must reach the wdio worker process, which
// inherits the harness environment.
const run = process.env.E2E_EMPTY_MIBS === "1" ? describe : describe.skip;

// launch-beta/20: with no configured MIB sources the frontend must still
// call mib_load so the backend swaps in the bundled standards. Before the
// fix the guard skipped the call: empty tree, "No MIBs loaded", numeric-only
// binding names.
run("Bundled MIBs on an empty config (launch-beta/20)", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("startup populates the tree from the bundled standards", async () => {
    // expandTo throws (node never appeared) when the fallback never ran;
    // node titles are dot-joined, so the mib-2 row is "mgmt.mib-2".
    await expandTo(CHAIN_TO_SYSTEM);
    await selectTreeNode("system");
  });

  it("walk bindings resolve names from the bundled MIBs", async () => {
    await typeOid("1.3.6.1.2.1.1");
    await browser.execute(() => document.body.click());
    await go("walk");
    const status = await waitForStatus(/^Walk complete: \d+ bindings$/);
    expect(status).toBe("Walk complete: 31 bindings");
    const body = (await (await $("[data-testid='results-body']")).getText()) ?? "";
    expect(body).toContain("sysDescr");
    expect(body).toContain("sysName");
  });
});
