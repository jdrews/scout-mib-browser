import { describe, it, expect, afterEach, vi } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/svelte";
import MenuBar from "./MenuBar.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

// axe (launch-beta/13): a ul[role="menu"] whose items sit in bare <li>
// elements violates aria-required-children (the li's implicit listitem is not
// an allowed menu child), aria-required-parent (the menuitem's nearest
// semantic ancestor is the li, not the menu) and listitem. The ARIA menu
// pattern wraps items in role="none" containers and labels non-interactive
// sections as role="group".

async function openMenu(container: HTMLElement, name: string) {
  await fireEvent.click(container.querySelector(`[data-testid="menu-${name}"]`)!);
  return container.querySelector('ul[role="menu"]')!;
}

describe("MenuBar menu role nesting (launch-beta/13)", () => {
  afterEach(() => {
    cleanup();
    S.fileMenuOpen = false;
    S.viewMenuOpen = false;
    S.settingsMenuOpen = false;
    mockedInvoke.mockReset();
  });

  it("every View menu child carries a menu-legal role", async () => {
    const { container } = render(MenuBar);
    const menu = await openMenu(container, "view");
    expect(menu).toBeTruthy();
    const items = menu.querySelectorAll(":scope > li");
    expect(items.length).toBe(2);
    for (const li of items) {
      expect(["none", "separator", "group"]).toContain(li.getAttribute("role"));
    }
  });

  it("menu items are owned by the menu, not by a listitem", async () => {
    const { container } = render(MenuBar);
    const menu = await openMenu(container, "view");
    const item = menu.querySelector('[data-testid="menu-mib-panel"]')!;
    expect(item.getAttribute("role")).toBe("menuitem");
    expect(item.parentElement!.getAttribute("role")).toBe("none");
  });

  it("the File menu wraps its items and separator the same way", async () => {
    const { container } = render(MenuBar);
    const menu = await openMenu(container, "file");
    const sep = menu.querySelector('[role="separator"]')!;
    expect(sep).toBeTruthy();
    for (const li of menu.querySelectorAll(":scope > li")) {
      expect(["none", "separator", "group"]).toContain(li.getAttribute("role"));
    }
  });

  it("the Settings log-level section is a named group", async () => {
    const { container } = render(MenuBar);
    const menu = await openMenu(container, "settings");
    const group = menu.querySelector('[role="group"]')!;
    expect(group.textContent).toContain("System Log Level");
    expect(group.getAttribute("aria-label")).toBe("System Log Level");
  });
});
