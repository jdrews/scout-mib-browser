const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Traps keyboard focus inside `container` (dialog pattern): moves focus into
 * the container, cycles Tab/Shift+Tab within it, and calls `onEscape` for
 * Escape. Returns a cleanup function that removes the listener.
 */
export function trapFocus(container: HTMLElement, onEscape?: () => void): () => void {
  function getFocusable(): HTMLElement[] {
    return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
      (el) => el.offsetParent !== null || el === document.activeElement
    );
  }

  const initial = container.querySelector<HTMLElement>("[data-autofocus]") ?? getFocusable()[0];
  initial?.focus();

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onEscape?.();
      return;
    }
    if (e.key !== "Tab") return;
    const items = getFocusable();
    if (!items.length) return;
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement as HTMLElement | null;
    if (e.shiftKey && (active === first || !container.contains(active))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (active === last || !container.contains(active))) {
      e.preventDefault();
      first.focus();
    }
  }

  container.addEventListener("keydown", onKeyDown);
  return () => container.removeEventListener("keydown", onKeyDown);
}
