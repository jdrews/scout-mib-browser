import { S } from "$lib/stores.svelte";
import type { TargetConfig } from "./types";
import { snmpConnect, logAppend } from "./tauriCommands";

export type ConnectionResult = "idle" | "success" | "error";

export interface ConnectionStateResult {
  connecting: boolean;
  result: ConnectionResult;
  errorMessage: string;
}

let _resultTimer: ReturnType<typeof setTimeout> | undefined;

export function clearResultTimer() {
  clearTimeout(_resultTimer);
}

function scheduleReset(delay: number, cb: () => void) {
  clearTimeout(_resultTimer);
  _resultTimer = setTimeout(cb, delay);
}

/**
 * Actionable failure message for a failed Test Connection: names the
 * host:port and suggests what to check. The raw transport error is kept in
 * the System Log separately (see runTestConnection's catch block).
 */
export function connectionFailureMessage(cfg: TargetConfig): string {
  return `Connection failed — no SNMP response from ${cfg.host}:${cfg.port}. Check the host/port and that the agent is listening.`;
}

export async function runTestConnection(cfg: TargetConfig): Promise<ConnectionStateResult> {
  if (!cfg.host.trim()) {
    S.statusText = "Enter a Target host to test connection";
    return { connecting: false, result: "idle", errorMessage: "" };
  }

  const isV3 = cfg.version === "v3";

  let connecting = true;
  let result: ConnectionResult = "idle";
  let errorMessage = "";

  clearResultTimer();
  S.connectionState = "connecting" as any;
  S.statusText = `Testing connection to ${cfg.host}:${cfg.port}...`;

  await Promise.resolve();

  try {
    await snmpConnect({
      host: cfg.host,
      port: cfg.port,
      version: cfg.version,
      community: isV3 ? undefined : cfg.community,
      v3_username: isV3 ? cfg.v3_username : undefined,
      v3_auth_protocol: isV3 ? cfg.v3_auth_protocol : undefined,
      v3_auth_passphrase: isV3 ? cfg.v3_auth_passphrase : undefined,
      v3_priv_protocol: isV3 ? cfg.v3_priv_protocol : undefined,
      v3_priv_passphrase: isV3 ? cfg.v3_priv_passphrase : undefined,
    });

    S.connectionState = "connected" as any;
    S.statusText = `Connected to ${cfg.host}:${cfg.port}`;
    result = "success";
    scheduleReset(2500, () => { result = "idle"; errorMessage = ""; });
  } catch (err) {
    S.connectionState = "disconnected" as any;
    const raw = typeof err === "string" ? err : (err as Error)?.message ?? String(err);
    // Preserve the raw transport error in the System Log for debugging; the
    // UI shows one actionable message instead.
    logAppend("ERROR", "scout.connection", `Test connection to ${cfg.host}:${cfg.port} failed: ${raw}`).catch(() => {});
    const msg = connectionFailureMessage(cfg);
    S.statusText = msg;
    result = "error";
    errorMessage = msg;
    scheduleReset(4000, () => { result = "idle"; errorMessage = ""; });
  } finally {
    connecting = false;
  }

  return { connecting, result, errorMessage };
}
