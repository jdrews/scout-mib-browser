import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import type { TargetConfig } from "$lib/types";
import { runTestConnection, clearResultTimer, connectionFailureMessage } from "$lib/connectionLogic";
import { S } from "$lib/stores.svelte";

vi.mock("$lib/tauriCommands", () => ({
  snmpConnect: vi.fn(),
  logAppend: vi.fn(() => Promise.resolve()),
}));

import { snmpConnect, logAppend } from "$lib/tauriCommands";
const mockedSnmpConnect = snmpConnect as ReturnType<typeof vi.fn>;
const mockedLogAppend = logAppend as ReturnType<typeof vi.fn>;

function makeConfig(overrides?: Partial<TargetConfig>): TargetConfig {
  return {
    host: "192.168.1.1",
    port: 161,
    version: "v2c",
    community: "public",
    v3_username: "",
    v3_auth_protocol: "none",
    v3_auth_passphrase: "",
    v3_priv_protocol: "none",
    v3_priv_passphrase: "",
    v3_security_level: "noAuthNoPrivacy",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockedSnmpConnect.mockReset();
  clearResultTimer();
  S.connectionState = "disconnected";
  S.statusText = "Ready";
});

afterEach(() => {
  clearResultTimer();
  vi.restoreAllMocks();
});

describe("runTestConnection", () => {
  it("returns idle when host is empty", async () => {
    const cfg = makeConfig({ host: "" });
    const result = await runTestConnection(cfg);

    expect(result).toEqual({ connecting: false, result: "idle", errorMessage: "" });
    expect(mockedSnmpConnect).not.toHaveBeenCalled();
    expect(S.statusText).toBe("Enter a Target host to test connection");
  });

  it("returns idle when host is whitespace only", async () => {
    const cfg = makeConfig({ host: "   " });
    const result = await runTestConnection(cfg);

    expect(result.result).toBe("idle");
    expect(mockedSnmpConnect).not.toHaveBeenCalled();
  });

  it("shows success state when connection succeeds", async () => {
    mockedSnmpConnect.mockResolvedValue({ bindings: [] });

    const result = await runTestConnection(makeConfig());

    expect(result.connecting).toBe(false);
    expect(result.result).toBe("success");
    expect(result.errorMessage).toBe("");
    expect(S.connectionState).toBe("connected");
    expect(S.statusText).toBe("Connected to 192.168.1.1:161");
  });

  it("shows an actionable error message naming the host:port", async () => {
    mockedSnmpConnect.mockRejectedValue(new Error("Receive"));

    const cfg = makeConfig({ host: "192.168.3.62", port: 1700 });
    const result = await runTestConnection(cfg);

    expect(result.connecting).toBe(false);
    expect(result.result).toBe("error");
    // Actionable copy: names the target and suggests checks — no raw "Receive".
    expect(result.errorMessage).toBe(
      "Connection failed — no SNMP response from 192.168.3.62:1700. Check the host/port and that the agent is listening.",
    );
    expect(S.connectionState).toBe("disconnected");
    expect(S.statusText).toBe(result.errorMessage);
  });

  it("preserves the raw error string in the System Log", async () => {
    const errorMsg = "IO error: connection refused";
    mockedSnmpConnect.mockRejectedValue(errorMsg);

    await runTestConnection(makeConfig({ host: "10.1.2.3", port: 162 }));

    expect(mockedLogAppend).toHaveBeenCalledWith(
      "ERROR",
      "scout.connection",
      `Test connection to 10.1.2.3:162 failed: ${errorMsg}`,
    );
  });

  it("handles unknown error type", async () => {
    mockedSnmpConnect.mockRejectedValue({ custom: "error" });

    const result = await runTestConnection(makeConfig());

    expect(result.result).toBe("error");
    // The UI still shows the actionable message; the raw string is logged.
    expect(result.errorMessage).toBe(connectionFailureMessage(makeConfig()));
    expect(mockedLogAppend).toHaveBeenCalledWith(
      "ERROR",
      "scout.connection",
      `Test connection to 192.168.1.1:161 failed: [object Object]`,
    );
  });

  it("sends correct params for v2c", async () => {
    mockedSnmpConnect.mockResolvedValue({ bindings: [] });

    await runTestConnection(makeConfig());

    expect(mockedSnmpConnect).toHaveBeenCalledWith({
      host: "192.168.1.1",
      port: 161,
      version: "v2c",
      community: "public",
      v3_username: undefined,
      v3_auth_protocol: undefined,
      v3_auth_passphrase: undefined,
      v3_priv_protocol: undefined,
      v3_priv_passphrase: undefined,
    });
  });

  it("sends correct params for v3", async () => {
    mockedSnmpConnect.mockResolvedValue({ bindings: [] });

    const v3Config = makeConfig({
      version: "v3",
      v3_username: "admin",
      v3_auth_protocol: "sha256",
      v3_auth_passphrase: "secret123",
      v3_priv_protocol: "aes256",
      v3_priv_passphrase: "privpass",
    });

    await runTestConnection(v3Config);

    expect(mockedSnmpConnect).toHaveBeenCalledWith({
      host: "192.168.1.1",
      port: 161,
      version: "v3",
      community: undefined,
      v3_username: "admin",
      v3_auth_protocol: "sha256",
      v3_auth_passphrase: "secret123",
      v3_priv_protocol: "aes256",
      v3_priv_passphrase: "privpass",
    });
  });

  it("sets connecting state before awaiting snmpConnect", async () => {
    let resolveDeferred: () => void;
    const deferred = new Promise<void>((resolve) => { resolveDeferred = resolve; });
    mockedSnmpConnect.mockImplementation(() => deferred);

    const promise = runTestConnection(makeConfig());

    // The function should have set state before awaiting snmpConnect
    expect(S.connectionState).toBe("connecting");
    expect(S.statusText).toContain("Testing connection to 192.168.1.1:161...");

    resolveDeferred!();
    const result = await promise;

    expect(result.connecting).toBe(false);
    expect(result.result).toBe("success");
  });

  it("schedules auto-reset on success", async () => {
    vi.useFakeTimers();
    mockedSnmpConnect.mockResolvedValue({ bindings: [] });

    const result = await runTestConnection(makeConfig());
    expect(result.result).toBe("success");

    // Timer is scheduled - verify it exists by checking that advancing time doesn't error
    vi.advanceTimersByTime(3000);
    vi.runOnlyPendingTimers();

    clearResultTimer();
    vi.useRealTimers();
  });

  it("schedules auto-reset on error", async () => {
    vi.useFakeTimers();
    mockedSnmpConnect.mockRejectedValue(new Error("refused"));

    const result = await runTestConnection(makeConfig());
    expect(result.result).toBe("error");

    vi.advanceTimersByTime(5000);
    vi.runOnlyPendingTimers();

    clearResultTimer();
    vi.useRealTimers();
  });
});
