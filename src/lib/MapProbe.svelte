<script lang="ts">
  let cache = $state(new Map<string, number>());
  // Reassignment is the tracked update path for $state Maps in this Svelte
  // version — see mapState.test.ts.
  export function put(key: string, v: number) {
    const next = new Map(cache);
    next.set(key, v);
    cache = next;
  }
</script>

<span data-testid="a">{cache.get("a") !== undefined ? String(cache.get("a")) : "none"}</span>
