<script lang="ts">
	import { onMount } from "svelte";
	import { get } from "svelte/store";

	import { matrixTriggerRoomUpdate } from "$lib/chats/api";
	import {
		logout as logoutSession,
		refreshRecoveryState,
		shellErrorMessage,
		shellRecoveryState,
		shellRefreshing,
		shellSelectedRoomId,
	} from "$lib/chats/shell";
	import { recoveryStateLabel } from "$lib/components/verification/helpers";

	let loggingOut = $state(false);
	let settingsMessage = $state("");

	async function refreshRooms() {
		if (get(shellRefreshing)) {
			return;
		}

		shellRefreshing.set(true);
		settingsMessage = "";
		shellErrorMessage.set("");

		try {
			await matrixTriggerRoomUpdate({
				selectedRoomId: get(shellSelectedRoomId) || null,
				includeSelectedMessages: false,
			});

			await refreshRecoveryState();

			settingsMessage = "Refresh requested.";
		} catch (error) {
			shellErrorMessage.set(error instanceof Error ? error.message : "Failed to trigger room refresh");
		} finally {
			shellRefreshing.set(false);
		}
	}

	async function logout() {
		loggingOut = true;
		settingsMessage = "";
		shellErrorMessage.set("");

		try {
			const error = await logoutSession();
			if (error) {
				shellErrorMessage.set(error);
			} else {
				window.location.replace("/");
			}
		} finally {
			loggingOut = false;
		}
	}
</script>

<section class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 space-y-4 max-h-[70vh] overflow-y-auto">
	<header>
		<p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Settings</p>
		<h2 class="h5">Account and Security</h2>
		<p class="text-xs text-surface-700-300">{recoveryStateLabel($shellRecoveryState)}</p>
	</header>

	{#if $shellErrorMessage}
		<p class="card p-2 text-sm preset-filled-error-500">{$shellErrorMessage}</p>
	{/if}

	{#if settingsMessage}
		<p class="text-xs text-surface-700-300">{settingsMessage}</p>
	{/if}

	<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
		<h3 class="h6">Sync</h3>
		<p class="text-xs text-surface-700-300">Request a room refresh from the server.</p>
		<button
			type="button"
			class="btn preset-tonal"
			onclick={refreshRooms}
			disabled={$shellRefreshing}
		>
			{#if $shellRefreshing}Refreshing...{:else}Refresh{/if}
		</button>
	</section>

	<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
		<h3 class="h6">Session</h3>
		<p class="text-xs text-surface-700-300">Sign out from the current account on this device.</p>
		<button
			type="button"
			class="btn preset-filled-error-500"
			onclick={logout}
			disabled={loggingOut}
		>
			{#if loggingOut}Logging out...{:else}Logout{/if}
		</button>
	</section>
</section>
