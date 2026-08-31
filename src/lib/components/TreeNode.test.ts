import { describe, it, expect, afterEach } from "vitest";
import { render } from "@testing-library/svelte";
import { tick } from "svelte";
import TreeNode from "./TreeNode.svelte";
import { S } from "$lib/stores.svelte";
import type { TreeNode as TreeNodeType } from "$lib/types";

function node(oid: string, name: string): TreeNodeType {
  return { oid, name, mibName: "TEST-MIB" };
}

afterEach(() => {
  S.treeFindOpen = false;
  S.treeFindQuery = "";
  S.treeFindOid = null;
});

describe("TreeNode find highlighting", () => {
  it("marks the query substring in every rendered entry whose name contains it", () => {
    S.treeFindOpen = true;
    S.treeFindQuery = "interfac";

    const match = render(TreeNode, { props: { node: node("1.3.6.1.2.1.2", "interfaces") } });
    const noMatch = render(TreeNode, { props: { node: node("1.3.6.1.2.1.2.1", "ifNumber") } });

    expect(match.container.querySelector("mark.find-mark")?.textContent).toBe("interfac");
    // The full name is preserved around the mark.
    expect(match.container.querySelector("[data-tree-node] span.truncate")?.textContent).toBe(
      "interfaces"
    );
    // A non-matching entry renders its plain name, unmarked.
    expect(noMatch.container.querySelector("mark.find-mark")).toBeNull();
  });

  it("grows the highlight as the query gets longer (interfac → interface)", async () => {
    S.treeFindOpen = true;
    S.treeFindQuery = "interfac";
    const view = render(TreeNode, { props: { node: node("1.3.6.1.2.1.2", "interfaces") } });

    expect(view.container.querySelector("mark.find-mark")?.textContent).toBe("interfac");

    // Typing the next character extends the highlight to the whole word stem.
    S.treeFindQuery = "interface";
    await tick();
    const mark = view.container.querySelector("mark.find-mark");
    expect(mark?.textContent).toBe("interface");
    expect(view.container.querySelector("[data-tree-node] span.truncate")?.textContent).toBe(
      "interfaces"
    );

    // A query that no longer matches clears the mark.
    S.treeFindQuery = "interzace";
    await tick();
    expect(view.container.querySelector("mark.find-mark")).toBeNull();
  });

  it("marks nothing while find is closed", () => {
    S.treeFindOpen = false;
    S.treeFindQuery = "interfac";

    const view = render(TreeNode, { props: { node: node("1.3.6.1.2.1.2", "interfaces") } });
    expect(view.container.querySelector("mark.find-mark")).toBeNull();
  });

  it("tints only the current hit row; other matches keep just their mark", () => {
    S.treeFindOpen = true;
    S.treeFindQuery = "inter";
    S.treeFindOid = "1.3.6.1.2.1.2";

    const hit = render(TreeNode, { props: { node: node("1.3.6.1.2.1.2", "interfaces") } });
    const otherMatch = render(TreeNode, { props: { node: node("1.3.6.1.2.1.9", "internet") } });

    expect(hit.container.querySelector("[data-tree-node]")?.classList.contains("find-hit")).toBe(
      true
    );
    // "internet" also matches "inter" but is not the current hit: marked, not tinted.
    expect(otherMatch.container.querySelector("mark.find-mark")?.textContent).toBe("inter");
    expect(otherMatch.container.querySelector("[data-tree-node]")?.classList.contains("find-hit")).toBe(
      false
    );
  });
});
