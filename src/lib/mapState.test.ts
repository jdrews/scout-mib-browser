import { describe, it, expect } from "vitest";
import { mount, unmount } from "svelte";
import MapProbe from "./MapProbe.svelte";

const tick = () => new Promise((r) => setTimeout(r, 20));

// Regression guard for the ResultsPane writability cache: in this Svelte
// version (5.20.x), an in-place `map.set(key, v)` on a $state Map does NOT
// invalidate renders that read `.has(key)`/`.get(key)`, while reassigning the
// binding with a new Map does. resolveWritability therefore rebuilds the map
// instead of mutating it — if this test starts failing after a Svelte upgrade,
// the mutation style may be safe again and the workaround can be dropped.
describe("$state Map updates", () => {
  it("reassigning the binding with a new Map re-renders readers", async () => {
    const target = document.createElement("div");
    document.body.appendChild(target);
    const comp = mount(MapProbe, { target });
    await tick();
    const span = () => target.querySelector("[data-testid='a']")?.textContent ?? "?";
    expect(span()).toBe("none");

    comp.put("a", 1);
    await tick();
    expect(span()).toBe("1");

    unmount(comp);
    target.remove();
  });
});
