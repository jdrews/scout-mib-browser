<script lang="ts">
  import { Check, X } from "lucide-svelte";
  import { S } from "$lib/stores.svelte";
  import { persistTargetConfig, configWrite } from "$lib/tauriCommands";
  import { runTestConnection, clearResultTimer } from "$lib/connectionLogic";

  let open = $derived(S.connectionPanelOpen);
  let cfg = $derived.by(() => ({ ...S.targetConfig }));
  let isV3 = $derived(cfg.version === "v3");

  let connecting = $state(false);
  let connectionResult: "idle" | "success" | "error" = $state("idle");
  let errorMessage = $state("");

  function close() {
    S.connectionPanelOpen = false;
  }

  function onSaveCredentialsChange() {
    // Persist the toggle itself so the opt-out survives restarts. Turning it
    // off also scrubs already-saved credentials from disk (backend).
    configWrite("ui.save_credentials", S.saveCredentials).catch((err) => {
      console.error("Failed to save save_credentials setting:", err);
    });
  }

  function updateField(field: string, value: string | number) {
    const next = { ...cfg, [field]: value };
    Object.assign(S.targetConfig, next);
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
      S.statusText = "Enter a Target host to test connection";
      return;
    }

    connecting = true;
    connectionResult = "idle";
    errorMessage = "";
    clearResultTimer();

    // Force UI flush before blocking call
    await new Promise((r) => setTimeout(r, 0));

    const result = await runTestConnection(cfg);
    connecting = false;
    connectionResult = result.result;
    errorMessage = result.errorMessage;
  }

  function handleBackdropClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("[data-connection-panel]")) return;
    close();
  }
</script>

{#if open}
  <dialog class="modal modal-open" onclick={handleBackdropClick}>
    <div data-connection-panel class="modal-box max-w-[480px]">
      <button aria-label="Close connection dialog" class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2" onclick={(e) => { e.stopPropagation(); close(); }}><X class="w-4 h-4" /></button>
      <h3 class="text-lg font-bold">Target Connection</h3>

      <div class="space-y-4 mt-4">
        <div>
          <label class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60 mb-1.5 block">SNMP Version</label>
          <div class="flex gap-1">
            {#each ["v1", "v2c", "v3"] as ver}
              <button
                class="btn btn-sm {cfg.version === ver ? 'btn-primary' : ''}"
                onclick={() => updateField("version", ver)}
              >
                SNMP {ver.toUpperCase()}
              </button>
            {/each}
          </div>
        </div>

        {#if !isV3}
          <div class="form-control">
            <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Community String</span></label>
            <input
              type="text"
              value={cfg.community}
              oninput={onCommunityInput}
              class="input input-bordered font-mono w-full"
            />
          </div>
        {/if}

        {#if isV3}
          <div class="form-control">
            <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Username</span></label>
            <input
              type="text"
              value={cfg.v3_username}
              oninput={onV3UsernameInput}
              class="input input-bordered font-mono w-full"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="form-control">
              <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Auth Protocol</span></label>
              <select
                value={cfg.v3_auth_protocol}
                onchange={onV3AuthProtocolChange}
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
                oninput={onV3AuthPassphraseInput}
                class="input input-bordered font-mono w-full"
              />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="form-control">
              <label class="label"><span class="label-text text-xs font-semibold uppercase tracking-wide text-base-content/60">Priv Protocol</span></label>
              <select
                value={cfg.v3_priv_protocol}
                onchange={onV3PrivProtocolChange}
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
                oninput={onV3PrivPassphraseInput}
                class="input input-bordered font-mono w-full"
              />
            </div>
          </div>
        {/if}

        <button
          data-testid="test-connection-btn"
          class="btn btn-block mt-2 {connectionResult === 'success' ? 'btn-success' : connectionResult === 'error' ? 'btn-error' : 'btn-primary'}"
          onclick={testConnection}
          disabled={connecting || !cfg.host.trim()}
        >
          {#if connecting}
            Testing...
          {:else if connectionResult === "success"}
            <Check class="w-4 h-4 inline-block" /> Connected
          {:else if connectionResult === "error"}
            <X class="w-4 h-4 inline-block" /> Failed
          {:else}
            Test Connection
          {/if}
        </button>

        {#if errorMessage}
          <p data-testid="connection-error" class="text-xs text-error font-mono bg-error/10 rounded px-2 py-1.5 break-all">{errorMessage}</p>
        {/if}

        <div class="flex items-start gap-2.5">
          <input
            id="save-credentials-toggle"
            type="checkbox"
            class="toggle toggle-sm toggle-primary mt-0.5 flex-shrink-0"
            data-testid="save-credentials-toggle"
            bind:checked={S.saveCredentials}
            onchange={onSaveCredentialsChange}
          />
          <div>
            <label for="save-credentials-toggle" class="text-xs font-semibold block mb-0.5">Save credentials</label>
            <p data-testid="credentials-note" class="text-xs text-base-content/60 leading-snug">
              {S.saveCredentials
                ? "Connection settings, including credentials, are saved to the local config file for convenience."
                : "Credentials will not be saved and must be re-entered on each launch."}
            </p>
          </div>
        </div>
      </div>
    </div>
  </dialog>
{/if}
