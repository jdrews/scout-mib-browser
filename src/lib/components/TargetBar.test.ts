import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, screen, waitFor, cleanup } from "@testing-library/svelte";
import TargetBar from "./TargetBar.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { ResultSet, VariableBinding } from "$lib/types";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

const binding: VariableBinding = { oid: "1.3.6.1.2.1.1.1.0", value: { OctetString: [114] } };
const nextBinding: VariableBinding = { oid: "1.3.6.1.2.1.1.2.0", value: { ObjectIdentifier: "1.3.6.1.4.1" } };

const ok = (bindings: VariableBinding[]): ResultSet => ({ bindings, partial: false });
const cancelled = (): ResultSet => ({
  bindings: [],
  partial: false,
  warnings: [{ kind: "cancelled", message: "Get Next cancelled" }],
});

function getNextCalls(): number {
  return mockedInvoke.mock.calls.filter((c) => c[0] === "snmp_get_next").length;
}

// The cancelled GetNext resolves Ok with the bindings the engine kept (08).
// When the run started from a typed OID (not the sequence cursor), the UI
// cleared the Result Set at run start — if the Ok landed before Stop's
// retire (CI load timing), the kept binding was wiped. Cancel must never
// discard data (launch-beta/08 contract, CI flake on PR #167).

describe("TargetBar cancelled GetNext keeps data (launch-beta/08)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_resolve_oid") return Promise.resolve(null);
      if (cmd === "mib_search") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    S.targetConfig.host = "192.0.2.1";
    S.snmpOperation = "getNext";
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.statusText = "Ready";
  });

  afterEach(() => {
    cleanup();
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.statusText = "Ready";
    S.snmpOperation = "get";
  });

  async function runGetNext(rendered: HTMLElement, oid: string) {
    const input = rendered.querySelector("[data-testid='oid-input']") as HTMLInputElement;
    input.value = oid;
    await fireEvent.input(input);
    await fireEvent.click(rendered.querySelector("[data-testid='go-btn']")!);
  }

  it("a cancelled fresh GetNext keeps the previous Result Set", async () => {
    let resolveSecond: ((r: ResultSet) => void) | null = null;
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_resolve_oid") return Promise.resolve(null);
      if (cmd === "mib_search") return Promise.resolve([]);
      if (cmd === "snmp_get_next") {
        if (getNextCalls() === 1) return Promise.resolve(ok([binding]));
        return new Promise<ResultSet>((res) => {
          resolveSecond = res;
        });
      }
      return Promise.resolve(undefined);
    });

    const { container } = render(TargetBar);
    await runGetNext(container, "1.3.6.1.2.1.1.1.0");
    await waitFor(() => expect(S.statusText).toBe("Get Next complete: 1 binding"));

    // Typed OID (not the cursor): a fresh sequence. The run is cancelled at
    // its first step — the engine kept nothing new, and the Ok lands while
    // the run is still current (Stop's retire has not run yet).
    await runGetNext(container, "1.3.6.1.2.1.1.5.0");
    await waitFor(() => expect(getNextCalls()).toBe(2));
    resolveSecond!(cancelled());
    await waitFor(() => expect(S.statusText).toMatch(/cancelled/i));

    expect(S.executionBindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.1.0"]);
    expect(S.executionResults?.bindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.1.0"]);
    expect(S.statusText).toContain("1 binding kept");
  });

  it("a retired fresh GetNext (Stop before the Ok) keeps the previous Result Set", async () => {
    let resolveSecond: ((r: ResultSet) => void) | null = null;
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_resolve_oid") return Promise.resolve(null);
      if (cmd === "mib_search") return Promise.resolve([]);
      if (cmd === "snmp_get_next") {
        if (getNextCalls() === 1) return Promise.resolve(ok([binding]));
        return new Promise<ResultSet>((res) => {
          resolveSecond = res;
        });
      }
      return Promise.resolve(undefined);
    });

    const { container } = render(TargetBar);
    await runGetNext(container, "1.3.6.1.2.1.1.1.0");
    await waitFor(() => expect(S.statusText).toBe("Get Next complete: 1 binding"));

    // The common local timing: Stop retires the run first, so the cancelled
    // Ok is dropped as stale. The fresh run's start-of-run clear must not
    // have destroyed the previous Result Set — the dropped outcome commits
    // nothing.
    await runGetNext(container, "1.3.6.1.2.1.1.5.0");
    await waitFor(() => expect(getNextCalls()).toBe(2));
    await fireEvent.click(container.querySelector("[data-testid='stop-btn']")!);
    await waitFor(() => expect(S.statusText).toMatch(/cancelled/i));
    resolveSecond!(cancelled());
    await new Promise((r) => setTimeout(r, 0));

    expect(S.executionBindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.1.0"]);
    expect(S.executionResults?.bindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.1.0"]);
  });

  it("a cancelled Get keeps the previous Result Set", async () => {
    let resolveGet: ((r: ResultSet) => void) | null = null;
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_resolve_oid") return Promise.resolve(null);
      if (cmd === "mib_search") return Promise.resolve([]);
      if (cmd === "snmp_get") {
        return new Promise<ResultSet>((res) => {
          resolveGet = res;
        });
      }
      return Promise.resolve(undefined);
    });

    const { container } = render(TargetBar);
    S.snmpOperation = "get";
    S.executionBindings.push(binding);
    S.executionResults = ok([binding]);
    await runGetNext(container, "1.3.6.1.2.1.1.5.0");
    await waitFor(() => expect(mockedInvoke.mock.calls.some((c) => c[0] === "snmp_get")).toBe(true));
    resolveGet!(ok([]));
    await new Promise((r) => setTimeout(r, 0));

    // A Get that answered with nothing (cancel semantics) must not erase
    // what was on screen.
    expect(S.executionBindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.1.0"]);
  });

  it("a successful fresh GetNext replaces the Result Set", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_resolve_oid") return Promise.resolve(null);
      if (cmd === "mib_search") return Promise.resolve([]);
      if (cmd === "snmp_get_next") return Promise.resolve(ok([nextBinding]));
      return Promise.resolve(undefined);
    });

    const { container } = render(TargetBar);
    S.executionBindings.push(binding);
    await runGetNext(container, "1.3.6.1.2.1.1.2.0");
    await waitFor(() => expect(S.statusText).toBe("Get Next complete: 1 binding"));

    // A real answer for a fresh sequence replaces: no merge of stale rows.
    expect(S.executionBindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.2.0"]);
  });

  it("a cancelled continuation keeps the engine's kept bindings", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_resolve_oid") return Promise.resolve(null);
      if (cmd === "mib_search") return Promise.resolve([]);
      if (cmd === "snmp_get_next") {
        if (getNextCalls() === 1) return Promise.resolve(ok([binding]));
        return Promise.resolve(cancelled());
      }
      return Promise.resolve(undefined);
    });

    const { container } = render(TargetBar);
    await runGetNext(container, "1.3.6.1.2.1.1.1.0");
    await waitFor(() => expect(S.statusText).toBe("Get Next complete: 1 binding"));

    // Continuation: the bar holds the cursor (the returned OID). The engine
    // keeps its sequence (one binding) — the UI must show exactly that,
    // not double it.
    await runGetNext(container, "1.3.6.1.2.1.1.2.0");
    await waitFor(() => expect(S.statusText).toMatch(/cancelled/i));
    expect(S.executionBindings.map((b) => b.oid)).toEqual(["1.3.6.1.2.1.1.1.0"]);
    expect(S.statusText).toContain("1 binding kept");
  });
});
