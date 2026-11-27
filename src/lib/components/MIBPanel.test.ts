import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";
import { tick } from "svelte";
import MIBPanel from "./MIBPanel.svelte";
import { S, requestTreeReveal } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { TreeNode } from "$lib/types";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

function node(oid: string, name: string, hasChildren = true): TreeNode {
  return { oid, name, mibName: "TEST-MIB", hasChildren };
}

// The chain the reveal walks: internet → dod → 6 → internet → mib-2, as the
// parent → children answers mib_children would return.
const childrenOf: Record<string, TreeNode[]> = {
  "1": [node("1.3", "dod")],
  "1.3": [node("1.3.6", "internet")],
  "1.3.6": [node("1.3.6.1", "mgmt")],
  "1.3.6.1": [node("1.3.6.1.2", "mib-2")],
};

// Reveal requests (results-row double-click / row menu) must drive the same
// expand-to-target walk the find bar uses, including when the panel was
// closed and the tree mounts fresh.

describe("MIBPanel reveal requests", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockImplementation((cmd: string, args: { oid?: string }) => {
      if (cmd === "mib_children") {
        return Promise.resolve(childrenOf[args?.oid ?? ""] ?? []);
      }
      return Promise.resolve(undefined);
    });
    S.treeData = [node("1", "internet")];
    S.oidNameMap = new Map<string, string>([
      ["1", "internet"],
      ["1.3", "dod"],
      ["1.3.6", "internet"],
      ["1.3.6.1", "mgmt"],
      ["1.3.6.1.2", "mib-2"],
    ]);
    S.mibPanelOpen = true;
    S.treeFindOpen = false;
    S.treeFindQuery = "";
    S.treeFindOid = null;
  });

  afterEach(() => {
    S.treeData = [];
    S.oidNameMap = new Map();
    S.mibPanelOpen = true;
    S.treeFindOid = null;
    S.revealInTree = null;
  });

  it("expands the branch to the requested OID and highlights it", async () => {
    // happy-dom has no scrollIntoView; goToMatch scrolls the target row.
    (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView ??= () => {};

    const view = render(MIBPanel);
    await tick();

    requestTreeReveal("1.3.6.1.2");

    await waitFor(
      () => {
        const rows = [...view.container.querySelectorAll("[data-tree-node] span.truncate")];
        const names = rows.map((el) => el.textContent);
        if (!names.some((n) => n?.includes("mib-2"))) throw new Error(`not revealed: ${names}`);
      },
      { timeout: 2000, interval: 50 }
    );
    expect(S.treeFindOid).toBe("1.3.6.1.2");
  });

  it("reveals when the panel opens in the same batch as the request", async () => {
    (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView ??= () => {};

    S.mibPanelOpen = false;
    const view = render(MIBPanel);
    await tick();
    expect(view.container.querySelector("[data-tree-node]")).toBeNull();

    // What ResultsPane's reveal does: open the panel, then request — one batch.
    S.mibPanelOpen = true;
    requestTreeReveal("1.3.6.1.2");

    await waitFor(
      () => {
        const names = [...view.container.querySelectorAll("[data-tree-node] span.truncate")].map(
          (el) => el.textContent
        );
        if (!names.some((n) => n?.includes("mib-2"))) throw new Error(`not revealed: ${names}`);
      },
      { timeout: 2000, interval: 50 }
    );
  });

  it("a repeated request for the same OID fires the walk again", async () => {
    (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView ??= () => {};

    const view = render(MIBPanel);
    await tick();

    requestTreeReveal("1.3.6.1.2");
    await waitFor(() => expect(S.treeFindOid).toBe("1.3.6.1.2"), { timeout: 2000, interval: 50 });

    // Collapse the branch behind the tree's back (unmount/remount drops the
    // expanded state), then request the same OID: the seq guard must treat it
    // as a new request, not a duplicate.
    S.mibPanelOpen = false;
    await tick();
    S.mibPanelOpen = true;
    S.treeFindOid = null;
    requestTreeReveal("1.3.6.1.2");

    await waitFor(
      () => {
        const names = [...view.container.querySelectorAll("[data-tree-node] span.truncate")].map(
          (el) => el.textContent
        );
        if (!names.some((n) => n?.includes("mib-2"))) throw new Error(`not re-revealed: ${names}`);
      },
      { timeout: 2000, interval: 50 }
    );
    expect(S.treeFindOid).toBe("1.3.6.1.2");
  });
});
