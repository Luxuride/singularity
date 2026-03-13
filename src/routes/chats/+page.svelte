<script lang="ts">
  import { onMount } from "svelte";
  import {
    matrixRecoverWithKey,
    matrixRecoveryStatus,
    matrixSessionStatus,
  } from "../../lib/auth/api";
  import {
    matrixAcceptSasVerification,
    matrixAcceptVerificationRequest,
    matrixGetChatMessages,
    matrixGetChats,
    matrixGetUserDevices,
    matrixGetVerificationFlow,
    matrixOwnVerificationStatus,
    matrixConfirmSasVerification,
    matrixRequestDeviceVerification,
    matrixStartSasVerification,
    matrixTriggerRoomUpdate,
  } from "../../lib/chats/api";
  import { subscribeToRoomUpdates } from "../../lib/chats/realtime";
  import type {
    MatrixChatMessage,
    MatrixChatSummary,
    MatrixDeviceInfo,
    MatrixOwnVerificationStatus,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
    MatrixVerificationFlowResponse,
  } from "../../lib/chats/types";
  import type { MatrixRecoveryState } from "../../lib/auth/types";

  let loading = $state(true);
  let loadingMessages = $state(false);
  let refreshing = $state(false);
  let errorMessage = $state("");
  let currentUserId = $state("");

  let chats = $state<MatrixChatSummary[]>([]);
  let selectedRoomId = $state("");
  let selectedRoomName = $state("");
  let selectedEncrypted = $state(false);

  let messages = $state<MatrixChatMessage[]>([]);
  let nextFrom = $state<string | null>(null);

  // Verification panel state
  let showVerificationPanel = $state(false);
  let ownVerification = $state<MatrixOwnVerificationStatus | null>(null);
  let selectedSenderDevices = $state<MatrixDeviceInfo[]>([]);
  let selectedSenderUserId = $state<string | null>(null);
  let loadingDevices = $state(false);
  let verificationError = $state("");
  let requestingVerification = $state<string | null>(null); // device_id being requested
  let activeVerificationFlow = $state<MatrixVerificationFlowResponse | null>(null);
  let activeVerificationDeviceId = $state<string | null>(null);
  let verificationActionPending = $state(false);
  let recoveryState = $state<MatrixRecoveryState | null>(null);
  let recoveryKey = $state("");
  let recoveryPending = $state(false);
  let recoveryMessage = $state("");
  let showRecoveryKeyForm = $state(false);

  onMount(() => {
    let unlisten = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: applyRoomUpsert,
        onRoomUpdated: applyRoomUpsert,
        onRoomRemoved: applyRoomRemoval,
        onSelectedRoomMessages: applySelectedRoomMessages,
        onVerificationStateChanged: (event) => {
          if (ownVerification) {
            ownVerification = { ...ownVerification, deviceVerified: event.verified };
          }
        },
      });

      await loadChats();
      await requestRefresh();
    })();

    return () => {
      unlisten();
    };
  });

  function applyRoomUpsert(chat: MatrixChatSummary) {
    const index = chats.findIndex((candidate) => candidate.roomId === chat.roomId);
    if (index >= 0) {
      chats = chats.map((candidate, candidateIndex) => (candidateIndex === index ? chat : candidate));
    } else {
      chats = [...chats, chat];
    }

    if (selectedRoomId === chat.roomId) {
      selectedRoomName = chat.displayName;
      selectedEncrypted = chat.encrypted;
    }

    if (!selectedRoomId && chats.length > 0) {
      openChat(chats[0]);
    }
  }

  function applyRoomRemoval(payload: MatrixRoomRemovedEvent) {
    chats = chats.filter((chat) => chat.roomId !== payload.roomId);

    if (selectedRoomId === payload.roomId) {
      selectedRoomId = "";
      selectedRoomName = "";
      selectedEncrypted = false;
      messages = [];
      nextFrom = null;
    }
  }

  function applySelectedRoomMessages(payload: MatrixSelectedRoomMessagesEvent) {
    if (payload.roomId !== selectedRoomId) {
      return;
    }

    messages = payload.messages;
    nextFrom = payload.nextFrom;
  }

  async function loadChats() {
    loading = true;
    errorMessage = "";

    try {
      const session = await matrixSessionStatus();
      if (!session.authenticated) {
        errorMessage = "You are not logged in. Use the login page first.";
        chats = [];
        return;
      }

      currentUserId = session.userId ?? "";
      await loadRecoveryStatus();

      chats = await matrixGetChats();

      if (chats.length > 0 && !selectedRoomId) {
        openChat(chats[0]);
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load chats";
    } finally {
      loading = false;
    }
  }

  async function requestRefresh() {
    if (refreshing) {
      return;
    }

    refreshing = true;
    errorMessage = "";

    try {
      await matrixTriggerRoomUpdate({
        selectedRoomId: selectedRoomId || undefined,
      });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to trigger room refresh";
    } finally {
      refreshing = false;
    }
  }

  function openChat(chat: MatrixChatSummary) {
    selectedRoomId = chat.roomId;
    selectedRoomName = chat.displayName;
    selectedEncrypted = chat.encrypted;
    messages = [];
    nextFrom = null;

    void loadMessages();
  }

  async function loadMessages() {
    if (!selectedRoomId) {
      return;
    }

    loadingMessages = true;
    errorMessage = "";

    try {
      const response = await matrixGetChatMessages({
        roomId: selectedRoomId,
        limit: 50,
      });

      messages = response.messages;
      nextFrom = response.nextFrom;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load messages";
    } finally {
      loadingMessages = false;
    }
  }

  async function loadOlder() {
    if (!selectedRoomId || !nextFrom) {
      return;
    }

    loadingMessages = true;
    errorMessage = "";

    try {
      const response = await matrixGetChatMessages({
        roomId: selectedRoomId,
        from: nextFrom,
        limit: 50,
      });

      messages = [...response.messages, ...messages];
      nextFrom = response.nextFrom;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load older messages";
    } finally {
      loadingMessages = false;
    }
  }

  function toTime(timestamp: number | null): string {
    if (!timestamp) {
      return "";
    }

    return new Date(timestamp).toLocaleString();
  }

  function decryptionLabel(status: MatrixChatMessage["decryptionStatus"]): string {
    if (status === "decrypted") {
      return "Decrypted";
    }

    if (status === "unableToDecrypt") {
      return "Unable to decrypt";
    }

    return "Plaintext";
  }

  function verificationLabel(status: MatrixChatMessage["verificationStatus"]): string {
    if (status === "verified") {
      return "Verified sender device";
    }

    if (status === "unverified") {
      return "Unverified sender device";
    }

    return "Verification unknown";
  }

  async function openVerificationPanel(senderUserId: string) {
    showVerificationPanel = true;
    selectedSenderUserId = senderUserId;
    selectedSenderDevices = [];
    activeVerificationFlow = null;
    activeVerificationDeviceId = null;
    verificationError = "";
    loadingDevices = true;

    try {
      if (!ownVerification) {
        ownVerification = await matrixOwnVerificationStatus();
      }

      const result = await matrixGetUserDevices(senderUserId);
      selectedSenderDevices = result.devices;
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to load device info";
    } finally {
      loadingDevices = false;
    }
  }

  async function openOwnVerificationPanel() {
    verificationError = "";

    try {
      if (!ownVerification) {
        ownVerification = await matrixOwnVerificationStatus();
      }

      const ownUserId = ownVerification.userId || currentUserId;
      if (!ownUserId) {
        throw new Error("Current user ID is not available");
      }

      await openVerificationPanel(ownUserId);
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to open verification panel";
    }
  }

  async function loadRecoveryStatus() {
    try {
      const status = await matrixRecoveryStatus();
      recoveryState = status.state;
    } catch {
      recoveryState = null;
    }
  }

  async function recoverKeys() {
    if (!recoveryKey.trim()) {
      recoveryMessage = "Recovery key is required.";
      return;
    }

    recoveryPending = true;
    recoveryMessage = "";
    verificationError = "";

    try {
      const response = await matrixRecoverWithKey({ recoveryKey });
      recoveryState = response.state;
      recoveryKey = "";
      showRecoveryKeyForm = false;
      recoveryMessage = "Recovery completed. Reloading messages…";
      await requestRefresh();
      await loadMessages();
    } catch (err) {
      recoveryMessage = err instanceof Error ? err.message : "Failed to recover encryption keys";
    } finally {
      recoveryPending = false;
    }
  }

  async function requestVerification(device: MatrixDeviceInfo) {
    requestingVerification = device.deviceId;
    verificationError = "";

    try {
      const response = await matrixRequestDeviceVerification(device.userId, device.deviceId);
      activeVerificationDeviceId = device.deviceId;
      activeVerificationFlow = await matrixGetVerificationFlow(response.userId, response.flowId);
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to start verification";
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
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to refresh verification flow";
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
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to accept verification request";
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
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to start SAS verification";
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
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to accept SAS verification";
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
    } catch (err) {
      verificationError = err instanceof Error ? err.message : "Failed to confirm SAS verification";
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

  function hasUnableToDecryptMessages(): boolean {
    return messages.some((message) => message.decryptionStatus === "unableToDecrypt");
  }
</script>

<main class="min-h-screen p-4 md:p-8">
  <section class="card p-4 md:p-6 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950">
    <header class="flex flex-wrap items-center justify-between gap-2">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Singularity</p>
        <h1 class="h3">Chats</h1>
      </div>

      <button type="button" class="btn preset-tonal" onclick={requestRefresh} disabled={loading || loadingMessages || refreshing}>
        Refresh
      </button>
    </header>

    {#if errorMessage}
      <p class="card p-3 text-sm preset-filled-error-500">{errorMessage}</p>
    {/if}

    {#if loading}
      <p class="card p-3 text-sm bg-surface-100-900">Loading chats...</p>
    {:else}
      <div class="grid gap-4 lg:grid-cols-[280px_1fr]">
        <aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto">
          {#if chats.length === 0}
            <p class="p-2 text-sm text-surface-700-300">No joined rooms found.</p>
          {:else}
            <ul class="space-y-1">
              {#each chats as chat (chat.roomId)}
                <li>
                  <button
                    type="button"
                    class="w-full text-left p-2 rounded hover:bg-surface-200-800 transition-colors"
                    class:bg-primary-100-900={chat.roomId === selectedRoomId}
                    onclick={() => openChat(chat)}
                  >
                    <p class="font-medium truncate">{chat.displayName}</p>
                    <p class="text-xs text-surface-700-300">
                      {chat.encrypted ? "Encrypted" : "Unencrypted"} • {chat.joinedMembers} members
                    </p>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </aside>

        <section class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto space-y-3">
          {#if !selectedRoomId}
            <p class="text-sm text-surface-700-300">Select a room to read messages.</p>
          {:else}
            <header class="flex items-center justify-between gap-2 sticky top-0 bg-surface-100-900 py-1">
              <div>
                <h2 class="h5">{selectedRoomName}</h2>
                <p class="text-xs text-surface-700-300">
                  {selectedEncrypted ? "Encrypted room" : "Unencrypted room"}
                </p>
              </div>

              <button
                type="button"
                class="btn preset-tonal"
                onclick={loadOlder}
                disabled={!nextFrom || loadingMessages}
              >
                {#if loadingMessages}Loading...{:else}Load Older{/if}
              </button>
            </header>

            {#if selectedEncrypted && hasUnableToDecryptMessages()}
              <section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-3">
                <div>
                  <p class="text-xs font-bold uppercase tracking-[0.18em] text-warning-700-300">Decryption Help</p>
                  <p class="text-sm">
                    Some messages could not be decrypted. You can verify another signed-in session to receive keys,
                    or import your recovery key.
                  </p>
                  <p class="text-xs text-surface-600-400 mt-1">{recoveryStateLabel(recoveryState)}</p>
                </div>

                <div class="flex flex-wrap gap-2">
                  <button type="button" class="btn preset-tonal text-xs" onclick={openOwnVerificationPanel}>
                    Verify Another Session
                  </button>
                  <button
                    type="button"
                    class="btn preset-tonal text-xs"
                    onclick={() => { showRecoveryKeyForm = !showRecoveryKeyForm; recoveryMessage = ""; }}
                  >
                    {showRecoveryKeyForm ? "Hide Recovery Key" : "Use Recovery Key"}
                  </button>
                </div>

                {#if showRecoveryKeyForm}
                  <div class="space-y-2">
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
                  </div>
                {/if}

                {#if recoveryMessage}
                  <p class="text-sm {recoveryMessage.includes('completed') ? 'text-success-700-300' : 'text-warning-700-300'}">
                    {recoveryMessage}
                  </p>
                {/if}
              </section>
            {/if}

            {#if messages.length === 0}
              <p class="text-sm text-surface-700-300">No messages yet.</p>
            {:else}
              <ul class="space-y-2">
                {#each messages as message, index (`${message.eventId ?? index}-${message.timestamp ?? 0}`)}
                  <li class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950">
                    <div class="flex items-center justify-between gap-2 text-xs text-surface-700-300 mb-1">
                      <button
                        type="button"
                        class="hover:text-primary-600-400 transition-colors"
                        onclick={() => openVerificationPanel(message.sender)}
                      >
                        {message.sender}
                      </button>
                      <span>{toTime(message.timestamp)}</span>
                    </div>
                    <p class="text-sm whitespace-pre-wrap break-words">{message.body}</p>
                    {#if message.encrypted}
                      <div class="mt-1 flex flex-wrap items-center gap-2 text-xs">
                        <span class="rounded px-2 py-0.5 bg-surface-200-800">{decryptionLabel(message.decryptionStatus)}</span>
                        <button
                          type="button"
                          class="rounded px-2 py-0.5 transition-colors hover:opacity-80"
                          class:bg-success-200-800={message.verificationStatus === "verified"}
                          class:bg-warning-200-800={message.verificationStatus === "unverified"}
                          class:bg-surface-200-800={message.verificationStatus === "unknown"}
                          onclick={() => openVerificationPanel(message.sender)}
                        >
                          {verificationLabel(message.verificationStatus)}
                        </button>
                      </div>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}
        </section>
      </div>
    {/if}
  </section>
</main>

{#if showVerificationPanel}
  <div
    class="fixed inset-0 z-50 flex items-end justify-end p-4 md:p-8 pointer-events-none"
    aria-hidden="true"
  >
    <div class="card p-5 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950 w-full max-w-md pointer-events-auto shadow-xl">
      <header class="flex items-center justify-between gap-2">
        <div>
          <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Device Verification</p>
          <h2 class="h5 truncate">{selectedSenderUserId}</h2>
        </div>
        <button
          type="button"
          class="btn-icon preset-tonal"
          onclick={() => { showVerificationPanel = false; }}
          aria-label="Close verification panel"
        >
          ✕
        </button>
      </header>

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
        <p class="text-sm text-surface-700-300">Loading devices…</p>
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
                  {requestingVerification === device.deviceId ? "Requesting…" : "Request Verification"}
                </button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}
