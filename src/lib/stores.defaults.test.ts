import { describe, it, expect } from "vitest";
import { S } from "$lib/stores.svelte";

describe("store defaults (launch-beta/04)", () => {
  it("does not persist credentials by default", () => {
    // Fresh installs must not write plaintext v3 passphrases to a shared
    // config file; persisting credentials is an explicit opt-in.
    expect(S.saveCredentials).toBe(false);
  });
});
