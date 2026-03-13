<script lang="ts">
  import { onMount } from "svelte";

  import { matrixRecoverWithKey, matrixRecoveryStatus } from "../../../../lib/auth/api";
  import {
    matrixAcceptSasVerification,
    matrixAcceptVerificationRequest,
    matrixConfirmSasVerification,
    matrixGetUserDevices,
    matrixGetVerificationFlow,
    matrixOwnVerificationStatus,
    matrixRequestDeviceVerification,
    matrixStartSasVerification,
  } from "../../../../lib/chats/api";
  import type { MatrixRecoveryState } from "../../../../lib/auth/types";
  import type {
    MatrixDeviceInfo,
    MatrixOwnVerificationStatus,
    MatrixVerificationFlowResponse,
  } from "../../../../lib/chats/types";

  let loading = $state(true);
  let errorMessage = $state("");

  let ownVerification = $state<MatrixOwnVerificationStatus | null>(null);
  let selectedSenderDevices = $state<MatrixDeviceInfo[]>([]);
  let selectedSenderUserId = $state<string | null>(null);
  let loadingDevices = $state(false);
  let verificationError = $state("");
  let requestingVerification = $state<string | null>(null);
  let activeVerificationFlow = $state<MatrixVerificationFlowResponse | null>(null);
  let verificationActionPending = $state(false);

  let recoveryState = $state<MatrixRecoveryState | null>(null);
  let recoveryKey = $state("");
  let recoveryPending = $state(false);
  let recoveryMessage = $state("");

  onMount(async () => {
    await loadSecurity();
  });

  async function loadSecurity() {
    loading = true;
    errorMessage = "";
    verificationError = "";

    try {
      ownVerification = await matrixOwnVerificationStatus();
      selectedSenderUserId = ownVerification.userId;
      const devices = await matrixGetUserDevices(ownVerification.userId);
      selectedSenderDevices = devices.devices;

      const status = await matrixRecoveryStatus();
      recoveryState = status.state;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load security settings";
    } finally {
      loading = false;
    }
  }

  async function recoverKeys() {
    if (!recoveryKey.trim()) {
      recoveryMessage = "Recovery key is required.";
      return;
    }

    recoveryPending = true;
    recoveryMessage = "";

    try {
      const response = await matrixRecoverWithKey({ recoveryKey });
      recoveryState = response.state;
      recoveryKey = "";
      recoveryMessage = "Recovery completed.";
    } catch (error) {
      recoveryMessage =
        error instanceof Error ? error.message : "Failed to recover encryption keys";
    } finally {
      recoveryPending = false;
    }
  }

  async function requestVerification(device: MatrixDeviceInfo) {
    requestingVerification = device.deviceId;
    verificationError = "";

    try {
      const response = await matrixRequestDeviceVerification(device.userId, device.deviceId);
      activeVerificationFlow = await matrixGetVerificationFlow(response.userId, response.flowId);
    } catch (error) {
      verificationError =
        error instanceof Error ? error.message : "Failed to start verification";
    } finally {
      requestingVerification = null;
    }
  }

  async function refreshVerificationFlow() {
    if (!activeVerificationFlow) {
      return;
    }

    verificationActionPending = true;
    verificationError = "";

    try {
      activeVerificationFlow = await matrixGetVerificationFlow(
        activeVerificationFlow.userId,
        activeVerificationFlow.flowId,
      );
    } catch (error) {
      verificationError =
        error instanceof Error ? error.message : "Failed to refresh verification flow";
    } finally {
      verificationActionPending = false;
    }
  }

  async function acceptVerificationRequest() {
    if (!activeVerificationFlow) {
      return;
    }

    verificationActionPending = true;
    verificationError = "";

    try {
      activeVerificationFlow = await matrixAcceptVerificationRequest(
        activeVerificationFlow.userId,
        activeVerificationFlow.flowId,
      );
    } catch (error) {
      verificationError =
        error instanceof Error ? error.message : "Failed to accept verification request";
    } finally {
      verificationActionPending = false;
    }
  }

  async function startSasVerification() {
    if (!activeVerificationFlow) {
      return;
    }

    verificationActionPending = true;
    verificationError = "";

    try {
      activeVerificationFlow = await matrixStartSasVerification(
        activeVerificationFlow.userId,
        activeVerificationFlow.flowId,
      );
    } catch (error) {
      verificationError =
        error instanceof Error ? error.message : "Failed to start SAS verification";
    } finally {
      verificationActionPending = false;
    }
  }

  async function acceptSasVerification() {
    if (!activeVerificationFlow) {
      return;
    }

    verificationActionPending = true;
    verificationError = "";

    try {
      activeVerificationFlow = await matrixAcceptSasVerification(
        activeVerificationFlow.userId,
        activeVerificationFlow.flowId,
      );
    } catch (error) {
      verificationError =
        error instanceof Error ? error.message : "Failed to accept SAS verification";
    } finally {
      verificationActionPending = false;
    }
  }

  async function confirmSasVerification() {
    if (!activeVerificationFlow) {
      return;
    }

    verificationActionPending = true;
    verificationError = "";

    try {
      activeVerificationFlow = await matrixConfirmSasVerification(
        activeVerificationFlow.userId,
        activeVerificationFlow.flowId,
      );

      if (selectedSenderUserId) {
        const result = await matrixGetUserDevices(selectedSenderUserId);
        selectedSenderDevices = result.devices;
      }
    } catch (error) {
      verificationError =
        error instanceof Error ? error.message : "Failed to confirm SAS verification";
    } finally {
      verificationActionPending = false;
    }
  }

  function flowStateLabel(flow: MatrixVerificationFlowResponse): string {
    if (flow.isDone) {
      return "Verification complete";
    }

    if (flow.isCancelled) {
      return "Verification cancelled";
    }

    if (flow.sasState) {
      return `SAS ${flow.sasState}`;
    }

    return `Request ${flow.requestState}`;
  }

  function deviceTrustClass(trust: MatrixDeviceInfo["trust"]): string {
    if (trust === "crossSigned" || trust === "locallyVerified") {
      return "bg-success-200-800";
    }

    if (trust === "notVerified") {
      return "bg-warning-200-800";
    }

    return "bg-surface-200-800";
  }

  function deviceTrustLabel(trust: MatrixDeviceInfo["trust"]): string {
    if (trust === "crossSigned") return "Cross-signing verified";
    if (trust === "locallyVerified") return "Locally verified";
    if (trust === "ownDevice") return "This device";
    return "Not verified";
  }

  function recoveryStateLabel(state: MatrixRecoveryState | null): string {
    if (state === "enabled") return "Recovery enabled";
    if (state === "incomplete") return "Recovery incomplete";
    if (state === "disabled") return "Recovery disabled";
    return "Recovery status unknown";
  }
</script>

<section class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 space-y-4 max-h-[70vh] overflow-y-auto">
  <header>
    <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Security</p>
    <h2 class="h5">Verification and Recovery</h2>
    <p class="text-xs text-surface-700-300">{recoveryStateLabel(recoveryState)}</p>
  </header>

  {#if loading}
    <p class="text-sm text-surface-700-300">Loading security details...</p>
  {:else}
    {#if errorMessage}
      <p class="card p-2 text-sm preset-filled-error-500">{errorMessage}</p>
    {/if}

    {#if ownVerification}
      <div class="text-xs p-2 rounded bg-surface-100-900 space-y-0.5">
        <p>Own device: <span class="font-mono">{ownVerification.deviceId}</span></p>
        <p>
          Status:
          <span
            class="rounded px-1.5 py-0.5 ml-1"
            class:bg-success-200-800={ownVerification.deviceVerified}
            class:bg-warning-200-800={!ownVerification.deviceVerified}
          >
            {ownVerification.deviceVerified ? "Verified" : "Unverified"}
          </span>
          {#if !ownVerification.crossSigningSetup}
            <span class="ml-1 text-surface-500-500">(Cross-signing not set up)</span>
          {/if}
        </p>
      </div>
    {/if}

    {#if verificationError}
      <p class="card p-2 text-sm preset-filled-error-500">{verificationError}</p>
    {/if}

    <section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
      <label class="label" for="recovery-key">Recovery Key</label>
      <textarea
        id="recovery-key"
        class="textarea"
        bind:value={recoveryKey}
        rows="3"
        placeholder="EsTc ..."
      ></textarea>
      <button
        type="button"
        class="btn preset-filled-primary-500 text-xs"
        onclick={recoverKeys}
        disabled={recoveryPending}
      >
        {#if recoveryPending}Recovering...{:else}Import Recovery Key{/if}
      </button>
      {#if recoveryMessage}
        <p class="text-sm {recoveryMessage.includes('completed') ? 'text-success-700-300' : 'text-warning-700-300'}">
          {recoveryMessage}
        </p>
      {/if}
    </section>

    {#if activeVerificationFlow}
      <section class="card p-3 preset-outlined-surface-300-700 bg-surface-100-900 space-y-3">
        <div class="flex items-center justify-between gap-2">
          <div>
            <p class="text-xs font-bold uppercase tracking-[0.18em] text-surface-600-400">Active Flow</p>
            <p class="text-sm font-medium">{flowStateLabel(activeVerificationFlow)}</p>
          </div>
          <button
            type="button"
            class="btn preset-tonal text-xs"
            onclick={refreshVerificationFlow}
            disabled={verificationActionPending}
          >
            Refresh Flow
          </button>
        </div>

        <p class="text-xs font-mono break-all text-surface-600-400">{activeVerificationFlow.flowId}</p>

        {#if activeVerificationFlow.message}
          <p class="text-xs text-warning-700-300">{activeVerificationFlow.message}</p>
        {/if}

        {#if activeVerificationFlow.decimals}
          <p class="text-sm font-mono tracking-[0.2em]">
            {activeVerificationFlow.decimals[0]} {activeVerificationFlow.decimals[1]} {activeVerificationFlow.decimals[2]}
          </p>
        {/if}

        {#if activeVerificationFlow.emojis.length > 0}
          <div class="grid grid-cols-4 gap-2 text-center sm:grid-cols-7">
            {#each activeVerificationFlow.emojis as emoji (`${emoji.symbol}-${emoji.description}`)}
              <div class="rounded bg-surface-200-800 p-2">
                <p class="text-lg">{emoji.symbol}</p>
                <p class="text-[10px] leading-tight">{emoji.description}</p>
              </div>
            {/each}
          </div>
        {/if}

        <div class="flex flex-wrap gap-2">
          {#if activeVerificationFlow.canAcceptRequest}
            <button
              type="button"
              class="btn preset-tonal text-xs"
              onclick={acceptVerificationRequest}
              disabled={verificationActionPending}
            >
              Accept Request
            </button>
          {/if}

          {#if activeVerificationFlow.canStartSas}
            <button
              type="button"
              class="btn preset-tonal text-xs"
              onclick={startSasVerification}
              disabled={verificationActionPending}
            >
              Start SAS
            </button>
          {/if}

          {#if activeVerificationFlow.canAcceptSas}
            <button
              type="button"
              class="btn preset-tonal text-xs"
              onclick={acceptSasVerification}
              disabled={verificationActionPending}
            >
              Accept SAS
            </button>
          {/if}

          {#if activeVerificationFlow.canConfirmSas}
            <button
              type="button"
              class="btn preset-filled-primary text-xs"
              onclick={confirmSasVerification}
              disabled={verificationActionPending}
            >
              Confirm Match
            </button>
          {/if}
        </div>
      </section>
    {/if}

    {#if loadingDevices}
      <p class="text-sm text-surface-700-300">Loading devices...</p>
    {:else if selectedSenderDevices.length === 0}
      <p class="text-sm text-surface-700-300">No devices found for this user.</p>
    {:else}
      <ul class="space-y-2">
        {#each selectedSenderDevices as device (device.deviceId)}
          <li class="card p-3 preset-outlined-surface-300-700 bg-surface-100-900 space-y-1">
            <div class="flex items-center justify-between gap-2">
              <div class="text-sm font-medium truncate">
                {device.displayName ?? device.deviceId}
                {#if device.displayName}
                  <span class="text-xs text-surface-600-400 ml-1 font-mono">{device.deviceId}</span>
                {/if}
              </div>
              <span class="rounded px-2 py-0.5 text-xs shrink-0 {deviceTrustClass(device.trust)}">
                {deviceTrustLabel(device.trust)}
              </span>
            </div>
            {#if device.ed25519Fingerprint}
              <p class="text-xs font-mono text-surface-600-400 break-all">{device.ed25519Fingerprint}</p>
            {/if}
            {#if device.trust === "notVerified"}
              <button
                type="button"
                class="btn preset-tonal text-xs w-full mt-1"
                disabled={requestingVerification === device.deviceId}
                onclick={() => requestVerification(device)}
              >
                {requestingVerification === device.deviceId ? "Requesting..." : "Request Verification"}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>
