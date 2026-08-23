import { AGENT_HOST, AGENT_PORT, clickPanelButton, panelLabelExists, readConfigFile, waitForAppReady } from "../support/helpers";

describe("Connection (target configuration)", () => {
  before(async () => {
    await waitForAppReady();
  });

  async function openModal() {
    await (await $("[data-testid='conn-gear']")).click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000 });
  }

  async function closeModal() {
    // The ✕ button is the first <button> inside the panel.
    const closeBtn = (await $$("[data-connection-panel] button"))[0];
    await closeBtn.click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000, reverse: true });
  }

  it("host/port inputs persist to config", async () => {
    // Use values distinct from the seeded config so the assertion is meaningful.
    await (await $("[data-testid='host-input']").setValue("192.0.2.10"));
    await (await $("[data-testid='port-input']").setValue("162"));
    await browser.pause(300);

    const toml = readConfigFile();
    expect(toml).toMatch(/host\s*=\s*"192\.0\.2\.10"/);
    expect(toml).toMatch(/port\s*=\s*162\b/);

    // Restore the seeded target for the remaining tests and later spec files.
    await (await $("[data-testid='host-input']").setValue(AGENT_HOST));
    await (await $("[data-testid='port-input']").setValue(String(AGENT_PORT)));
  });

  it("connection modal opens from gear and Settings menu", async () => {
    await openModal();
    await expect(await $("[data-connection-panel]")).toBeExisting();
    await closeModal();

    await (await $("[data-testid='menu-settings']")).click();
    await (await $("[data-testid='menu-connection']")).click();
    await expect(await $("[data-connection-panel]")).toBeExisting();
    await closeModal();
  });

  it("version toggle swaps credential fields", async () => {
    await openModal();

    // The driver cannot select by text; panelLabelExists/clickPanelButton are
    // execute-based (see helpers.ts).
    expect(await panelLabelExists("Community String")).toBe(true);

    await clickPanelButton("SNMP V3");
    expect(await panelLabelExists("Community String")).toBe(false);
    for (const label of ["Username", "Auth Protocol", "Auth Passphrase", "Priv Protocol", "Priv Passphrase"]) {
      expect(await panelLabelExists(label)).toBe(true);
    }

    // Restore v2c for the Test Connection cases below.
    await clickPanelButton("SNMP V2C");
    expect(await panelLabelExists("Community String")).toBe(true);
    await closeModal();
  });

  it("Test Connection succeeds against snmpsim", async () => {
    await openModal();
    const btn = await $("[data-connection-panel] .btn-block");
    await btn.click();

    await browser.waitUntil(
      async () => {
        const t = (await btn.getText()) ?? "";
        return t.includes("Connected") || t.includes("Failed");
      },
      { timeout: 45000, interval: 500, timeoutMsg: "Test Connection never finished" }
    );
    expect((await btn.getText()) ?? "").toContain("✓ Connected");

    const indicator = (await (await $("[data-testid='conn-indicator']").getText())) ?? "";
    expect(indicator).toContain("Connected");
    await closeModal();
  });

  it("Test Connection failure is actionable (names host:port, suggests checks)", async () => {
    // Point at an unused local port — UDP replies with port-unreachable.
    await (await $("[data-testid='port-input']").setValue("11699"));
    await openModal();
    const btn = await $("[data-connection-panel] .btn-block");
    await btn.click();

    await browser.waitUntil(
      async () => ((await btn.getText()) ?? "").includes("Failed"),
      { timeout: 60000, interval: 500, timeoutMsg: "Test Connection did not fail" }
    );
    expect((await btn.getText()) ?? "").toContain("Failed");

    // The error paragraph is the actionable message: it names the exact
    // host:port and suggests what to check (no raw transport string).
    let errMsg = "";
    for (const p of await $$("[data-connection-panel] p")) {
      const t = (await p.getText()) ?? "";
      if (t.startsWith("Connection failed — no SNMP response from")) errMsg = t;
    }
    expect(errMsg).toContain(`no SNMP response from ${AGENT_HOST}:11699`);
    expect(errMsg).toContain("Check the host/port and that the agent is listening");

    // Restore the seeded port.
    await closeModal();
    await (await $("[data-testid='port-input']").setValue(String(AGENT_PORT)));
  });
});
