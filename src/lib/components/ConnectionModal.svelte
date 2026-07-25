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
  <dialog class="modal modal-open" on:click={handleBackdropClick}>
    <div data-connection-panel class="modal-box max-w-[480px]">
      <form method="dialog">
        <button class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2">✕</button>
      </form>
      <h3 class="text-lg font-bold">Target Connection</h3>

      <div class="space-y-4 mt-4">
        <!-- Version selector -->
        <div>
          <label class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60 mb-1.5 block">SNMP Version</label>
          <div class="flex gap-1">
            {#each ["v1", "v2c", "v3"] as ver}
              <button
                class="btn btn-sm {cfg.version === ver ? 'btn-primary' : ''}"
                on:click={() => updateField("version", ver)}
              >
                SNMP {ver.toUpperCase()}
              </button>
            {/each}
          </div>
        </div>

        <!-- v1/v2c Community string -->
        {#if !isV3}
          <div class="form-control">
            <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Community String</span></label>
            <input
              type="text"
              value={cfg.community}
              on:input={onCommunityInput}
              class="input input-bordered font-mono w-full"
            />
          </div>
        {/if}

        <!-- v3 fields -->
        {#if isV3}
          <div class="form-control">
            <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Username</span></label>
            <input
              type="text"
              value={cfg.v3_username}
              on:input={onV3UsernameInput}
              class="input input-bordered font-mono w-full"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="form-control">
              <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Auth Protocol</span></label>
              <select
                value={cfg.v3_auth_protocol}
                on:change={onV3AuthProtocolChange}
                class="select select-bordered w-full"
              >
                {#each ["none", "md5", "sha1", "sha224", "sha256", "sha384", "sha512"] as proto}
                  <option value={proto}>{proto.toUpperCase()}</option>
                {/each}
              </select>
            </div>
            <div class="form-control">
              <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Auth Passphrase</span></label>
              <input
                type="password"
                value={cfg.v3_auth_passphrase}
                on:input={onV3AuthPassphraseInput}
                class="input input-bordered font-mono w-full"
              />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="form-control">
              <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Priv Protocol</span></label>
              <select
                value={cfg.v3_priv_protocol}
                on:change={onV3PrivProtocolChange}
                class="select select-bordered w-full"
              >
                {#each ["none", "des", "aes128", "aes192", "aes256"] as proto}
                  <option value={proto}>{proto.toUpperCase()}</option>
                {/each}
              </select>
            </div>
            <div class="form-control">
              <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Priv Passphrase</span></label>
              <input
                type="password"
                value={cfg.v3_priv_passphrase}
                on:input={onV3PrivPassphraseInput}
                class="input input-bordered font-mono w-full"
              />
            </div>
          </div>
        {/if}

        <!-- Test connection button -->
        <button
          class="btn btn-primary btn-block mt-2"
          on:click={testConnection}
          disabled={connecting || !cfg.host.trim()}
        >
          {connecting ? "Testing..." : "Test Connection"}
        </button>

        <p class="text-xs text-base-content/60 italic">Credentials are not persisted beyond the current session. Re-enter on each launch.</p>
      </div>
    </div>
  </dialog>
{/if}
