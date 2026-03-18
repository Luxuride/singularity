<script lang="ts">
  import { onMount } from "svelte";

  import { matrixRecoverWithKey, matrixRecoveryStatus } from "$lib/auth/api";
  import {
    matrixAcceptSasVerification,
    matrixAcceptVerificationRequest,
    matrixConfirmSasVerification,
    matrixGetUserDevices,
    matrixGetVerificationFlow,
    matrixOwnVerificationStatus,
    matrixRequestDeviceVerification,
    matrixStartSasVerification,
  } from "$lib/chats/api";
  import type { MatrixRecoveryState } from "$lib/auth/types";
  import type {
    MatrixDeviceInfo,
    MatrixOwnVerificationStatus,
    MatrixVerificationFlowResponse,
  } from "$lib/chats/types";
  import OwnDeviceInfo from "$lib/components/verification/OwnDeviceInfo.svelte";
  import DeviceList from "$lib/components/verification/DeviceList.svelte";
  import VerificationFlow from "$lib/components/verification/VerificationFlow.svelte";
  import RecoveryKeyImport from "$lib/components/recovery/RecoveryKeyImport.svelte";
  import { recoveryStateLabel } from "$lib/components/verification/helpers";

  let loading = $state(true);
  let errorMessage = $state("");

  let ownVerification = $state<MatrixOwnVerificationStatus | null>(null);
  let selectedSenderDevices = $state<MatrixDeviceInfo[]>([]);
  let selectedSenderUserId = $state<string | null>(null);
  let requestingVerification = $state<string | null>(null);
  let activeVerificationFlow = $state<MatrixVerificationFlowResponse | null>(null);
  let verificationActionPending = $state(false);
  let verificationError = $state("");

  let recoveryState = $state<MatrixRecoveryState | null>(null);
  let recoveryMessage = $state("");
  let recoveryPending = $state(false);

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

  async function recoverKeys(key: string) {
    if (!key.trim()) {
      recoveryMessage = "Recovery key is required.";
      return;
    }

    recoveryPending = true;
    recoveryMessage = "";

    try {
      const response = await matrixRecoverWithKey({ recoveryKey: key });
      recoveryState = response.state;
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

    <OwnDeviceInfo {ownVerification} {loading} />

    {#if verificationError}
      <p class="card p-2 text-sm preset-filled-error-500">{verificationError}</p>
    {/if}

    <RecoveryKeyImport
      {recoveryState}
      pending={recoveryPending}
      message={recoveryMessage}
      onImport={recoverKeys}
    />

    <VerificationFlow
      flow={activeVerificationFlow}
      pending={verificationActionPending}
      error={verificationError}
      onRefresh={refreshVerificationFlow}
      onAcceptRequest={acceptVerificationRequest}
      onStartSas={startSasVerification}
      onAcceptSas={acceptSasVerification}
      onConfirmSas={confirmSasVerification}
    />

    <DeviceList
      devices={selectedSenderDevices}
      loading={false}
      requestingDeviceId={requestingVerification}
      onVerify={requestVerification}
    />
  {/if}
</section>
