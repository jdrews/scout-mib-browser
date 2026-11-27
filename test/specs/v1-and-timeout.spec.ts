import {
  CHAIN_TO_INTERFACES,
  clickPanelButton,
  expandTo,
  go,
  restoreTargetPort,
  selectTreeNode,
  setTargetPort,
  typeOid,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

// Black-hole UDP listener started by scripts/test-e2e.sh: accepts the
// socket, never answers (launch-beta/28).
const BLACKHOLE_PORT = Number(process.env.E2E_BLACKHOLE_PORT || 11698);

// v1 + timeout settings leak through the persisted target config, so every
// mutation here is undone before the next spec file runs.
describe("v1 walks and request timeouts (launch-beta/27, 28)", () => {
  before(async () => {
    await waitForAppReady();
  });

  async function openModal() {
    await (await $("[data-testid='conn-gear']")).click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000 });
  }

  async function closeModal() {
    const closeBtn = (await $$("[data-connection-panel] button"))[0];
    await closeBtn.click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000, reverse: true });
  }

  async function setVersion(button: string) {
    await openModal();
    await clickPanelButton(button);
    await closeModal();
  }

  it("Get Table completes against an SNMPv1 target", async () => {
    // SNMPv1 has no GetBulk (RFC 1157): the engine must walk with GetNext.
    // Before the fix this hung at "Fetching table ifTable…" forever.
    await setVersion("SNMP V1");
    try {
      await expandTo(CHAIN_TO_INTERFACES);
      await selectTreeNode("ifTable");
      await go("getTable");

      const status = await waitForStatus(/^Table complete: \d+ rows?, \d+ columns?$/, 60000);
      expect(status).toBe("Table complete: 2 rows, 22 columns");
    } finally {
      await setVersion("SNMP V2C");
    }
  });

  it("Get against a silent target ends in a timeout status", async () => {
    // 1s deadline, 4 attempts, backoff 1s+2s+4s: terminal status in ~11s.
    await openModal();
    await (await $("[data-testid='timeout-input']")).setValue("1");
    await closeModal();

    await setTargetPort(BLACKHOLE_PORT);
    try {
      await typeOid("1.3.6.1.2.1.1.5.0");
      await go("get");

      const status = await waitForStatus(/No response received from Target/, 40000);
      expect(status).toContain("No response received from Target");
    } finally {
      // Restore the default deadline and the primary agent for later specs.
      await openModal();
      await (await $("[data-testid='timeout-input']")).setValue("");
      await closeModal();
      await restoreTargetPort();
    }
  });
});
