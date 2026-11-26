import type { LogLevel } from "./types";

// System Log level filtering (launch-beta/10). Extracted from SystemLogPane
// so the severity semantics are testable: the shipped "Info+" predicate
// returned true for every entry, leaking DEBUG/TRACE into Info+.

/** Severity rank per level; unknown levels rank as INFO (visible at Info+). */
const SEVERITY: Record<string, number> = {
  TRACE: 0,
  DEBUG: 1,
  INFO: 2,
  WARN: 3,
  ERROR: 4,
};

/** True when an entry of `level` is visible under `filter`. */
export function passesLogFilter(level: string, filter: LogLevel): boolean {
  if (filter === "all") return true;
  const rank = SEVERITY[level.toUpperCase()] ?? SEVERITY.INFO;
  return rank >= SEVERITY[filter.toUpperCase()];
}
