<script lang="ts">
  import { connectionPanelOpen, targetConfig, statusText, connectionState } from "$lib/stores";
  import { snmpConnect, persistTargetConfig } from "$lib/tauriCommands";

  $: open = $connectionPanelOpen;
  $: cfg = $targetConfig;
  $: isV3 = cfg.version === "v3";

  let connecting = false;

  function close() {
    $connectionPanelOpen = false;
  }

  function updateField(field: string, value: string | number) {
    const next = { ...cfg, [field]: value };
    $targetConfig = next;
    persistTargetConfig(next);
  }

  function onCommunityInput(e: Event) {
    updateField("community", (e.target as HTMLInputElement).value);
  }

  function onV3UsernameInput(e: Event) {
    updateField("v3_username", (e.target as HTMLInputElement).value);
  }

  function onV3AuthProtocolChange(e: Event) {
    updateField("v3_auth_protocol", (e.target as HTMLSelectElement).value);
  }

  function onV3AuthPassphraseInput(e: Event) {
    updateField("v3_auth_passphrase", (e.target as HTMLInputElement).value);
  }

  function onV3PrivProtocolChange(e: Event) {
    updateField("v3_priv_protocol", (e.target as HTMLSelectElement).value);
  }

  function onV3PrivPassphraseInput(e: Event) {
    updateField("v3_priv_passphrase", (e.target as HTMLInputElement).value);
  }

  async function testConnection() {
    if (!cfg.host.trim()) {
      $statusText = "Enter a Target host to test connection";
      return;
    }

    connecting = true;
    $connectionState = "connecting";
    $statusText = `Testing connection to ${cfg.host}:${cfg.port}...`;

    try {
      await snmpConnect({
        host: cfg.host,
        port: cfg.port,
        version: cfg.version,
        community: isV3 ? undefined : cfg.community,
        v3_username: isV3 ? cfg.v3_username : undefined,
        v3_auth_protocol: isV3 ? cfg.v3_auth_protocol : undefined,
        v3_auth_passphrase: isV3 ? cfg.v3_auth_passphrase : undefined,
        v3_priv_protocol: isV3 ? cfg.v3_priv_protocol : undefined,
        v3_priv_passphrase: isV3 ? cfg.v3_priv_passphrase : undefined,
      });
      $connectionState = "connected";
      $statusText = `Connected to ${cfg.host}:${cfg.port}`;
    } catch (err) {
      $connectionState = "disconnected";
      $statusText = `Connection failed: ${err}`;
    } finally {
      connecting = false;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("[data-connection-panel]")) return;
    close();
  }
</script>

{#if open}
  <div class="fixed inset-0 z-[2000] flex items-center justify-center bg-black/50" on:click={handleBackdropClick}>
    <div data-connection-panel class="bg-base-00 border border-base-01 rounded-xl shadow-2xl w-[480px] max-h-[90vh] overflow-y-auto">
      <div class="flex items-center justify-between px-5 py-3 border-b border-base-01">
        <h2 class="text-sm font-semibold text-text">Target Connection</h2>
        <button class="text-overlay hover:text-text cursor-pointer" on:click={close}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
            <path d="M3.5 3.5l9 9m0-9l-9 9" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round"/>
          </svg>
        </button>
      </div>

      <div class="px-5 py-4 space-y-4">
        <!-- Version selector -->
        <div>
          <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">SNMP Version</label>
          <div class="flex gap-2">
            {#each ["v1", "v2c", "v3"] as ver}
              <button
                class="flex-1 px-3 py-1.5 text-[13px] rounded border cursor-pointer transition-colors"
                class:border-blue={cfg.version === ver}
                class:bg-base-01={cfg.version === ver}
                class:text-text={cfg.version === ver}
                class:border-base-01={cfg.version !== ver}
                class:text-overlay={cfg.version !== ver}
                on:click={() => updateField("version", ver)}
              >
                SNMP {ver.toUpperCase()}
              </button>
            {/each}
          </div>
        </div>

        <!-- v1/v2c Community string -->
        {#if !isV3}
          <div>
            <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">Community String</label>
            <input
              type="text"
              value={cfg.community}
              on:input={onCommunityInput}
              class="w-full bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] font-mono rounded outline-none focus:border-blue"
            />
          </div>
        {/if}

        <!-- v3 fields -->
        {#if isV3}
          <div>
            <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">Username</label>
            <input
              type="text"
              value={cfg.v3_username}
              on:input={onV3UsernameInput}
              class="w-full bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] font-mono rounded outline-none focus:border-blue"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">Auth Protocol</label>
              <select
                value={cfg.v3_auth_protocol}
                on:change={onV3AuthProtocolChange}
                class="w-full bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] rounded outline-none focus:border-blue"
              >
                {#each ["none", "md5", "sha1", "sha224", "sha256", "sha384", "sha512"] as proto}
                  <option value={proto}>{proto.toUpperCase()}</option>
                {/each}
              </select>
            </div>
            <div>
              <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">Auth Passphrase</label>
              <input
                type="password"
                value={cfg.v3_auth_passphrase}
                on:input={onV3AuthPassphraseInput}
                class="w-full bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] font-mono rounded outline-none focus:border-blue"
              />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">Priv Protocol</label>
              <select
                value={cfg.v3_priv_protocol}
                on:change={onV3PrivProtocolChange}
                class="w-full bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] rounded outline-none focus:border-blue"
              >
                {#each ["none", "des", "aes128", "aes192", "aes256"] as proto}
                  <option value={proto}>{proto.toUpperCase()}</option>
                {/each}
              </select>
            </div>
            <div>
              <label class="block text-[11px] font-semibold uppercase tracking-wide text-overlay mb-1.5">Priv Passphrase</label>
              <input
                type="password"
                value={cfg.v3_priv_passphrase}
                on:input={onV3PrivPassphraseInput}
                class="w-full bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] font-mono rounded outline-none focus:border-blue"
              />
            </div>
          </div>
        {/if}

        <!-- Test connection button -->
        <div class="pt-2 flex gap-2">
          <button
            class="flex-1 bg-blue text-base-00 border-none px-4 py-2 text-[13px] font-semibold rounded cursor-pointer hover:bg-sapphire transition-colors disabled:opacity-50"
            on:click={testConnection}
            disabled={connecting || !cfg.host.trim()}
          >
            {connecting ? "Testing..." : "Test Connection"}
          </button>
        </div>

        <p class="text-[11px] text-overlay italic">Credentials are not persisted beyond the current session. Re-enter on each launch.</p>
      </div>
    </div>
  </div>
{/if}
