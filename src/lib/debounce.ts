// Trailing-edge debounce (launch-beta/10). Used to coalesce the
// target-config persist that used to fire on every keystroke.

export interface Debounced<A extends unknown[]> {
  (...args: A): void;
  /** Invoke a pending call now and clear the timer. */
  flush(): void;
  /** Drop a pending call. */
  cancel(): void;
}

export function debounce<A extends unknown[]>(
  fn: (...args: A) => void,
  ms: number,
): Debounced<A> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastArgs: A | null = null;

  const d = ((...args: A) => {
    lastArgs = args;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      const a = lastArgs;
      lastArgs = null;
      if (a) fn(...a);
    }, ms);
  }) as Debounced<A>;

  d.flush = () => {
    if (timer) clearTimeout(timer);
    timer = null;
    const a = lastArgs;
    lastArgs = null;
    if (a) fn(...a);
  };
  d.cancel = () => {
    if (timer) clearTimeout(timer);
    timer = null;
    lastArgs = null;
  };
  return d;
}
