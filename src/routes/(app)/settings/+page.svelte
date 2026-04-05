<script lang="ts">
	import { onMount } from "svelte";
	import { get } from "svelte/store";

	import { matrixLogout, matrixRecoveryStatus } from "$lib/auth/api";
	import { matrixTriggerRoomUpdate } from "$lib/chats/api";
	import { matrixGetMediaSettings, matrixSetMediaSettings } from "$lib/settings/api";
	import {
		shellChats,
		shellCurrentUserId,
		shellErrorMessage,
		shellPickerCustomEmoji,
		shellRecoveryState,
		shellRefreshing,
		shellRootScopedRooms,
		shellRootSpaces,
		shellSelectedRootSpaceId,
		shellSelectedRoomId,
	} from "$lib/chats/shell";
	import { recoveryStateLabel } from "$lib/components/verification/helpers";

	let loggingOut = $state(false);
	let settingsMessage = $state("");
	let useAssetStorage = $state(false);
	let mediaSettingsPending = $state(false);
	let mediaSettingsLoaded = $state(false);
	let mediaSettingsMessage = $state("");

	onMount(() => {
		void loadMediaSettings();
	});

	async function loadMediaSettings() {
		mediaSettingsPending = true;
		mediaSettingsMessage = "";

		try {
			const response = await matrixGetMediaSettings();
			useAssetStorage = response.useAssetStorage;
			mediaSettingsLoaded = true;
		} catch (error) {
			mediaSettingsMessage =
				error instanceof Error ? error.message : "Failed to load media settings";
		} finally {
			mediaSettingsPending = false;
		}
	}

	async function updateMediaStorageMode(event: Event) {
		if (!(event.currentTarget instanceof HTMLInputElement)) {
			return;
		}

		if (!mediaSettingsLoaded || mediaSettingsPending) {
			event.currentTarget.checked = useAssetStorage;
			return;
		}

		const previous = useAssetStorage;
		const next = event.currentTarget.checked;

		mediaSettingsPending = true;
		mediaSettingsMessage = "";

		try {
			const response = await matrixSetMediaSettings({
				useAssetStorage: next,
			});

			useAssetStorage = response.useAssetStorage;
			mediaSettingsMessage = response.useAssetStorage
				? "Asset storage enabled for chat images."
				: "In-memory media protocol enabled for chat images.";

			await matrixTriggerRoomUpdate({
				selectedRoomId: get(shellSelectedRoomId) || undefined,
				includeSelectedMessages: true,
			});
		} catch (error) {
			useAssetStorage = previous;
			event.currentTarget.checked = previous;
			mediaSettingsMessage =
				error instanceof Error ? error.message : "Failed to update media settings";
		} finally {
			mediaSettingsPending = false;
		}
	}

	async function refreshRooms() {
		if (get(shellRefreshing)) {
			return;
		}

		shellRefreshing.set(true);
		settingsMessage = "";
		shellErrorMessage.set("");

		try {
			await matrixTriggerRoomUpdate({
				selectedRoomId: get(shellSelectedRoomId) || undefined,
			});

			try {
				const recovery = await matrixRecoveryStatus();
				shellRecoveryState.set(recovery.state);
			} catch {
				shellRecoveryState.set(null);
			}

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
			await matrixLogout();
			shellChats.set([]);
			shellRootSpaces.set([]);
			shellRootScopedRooms.set([]);
			shellSelectedRoomId.set("");
			shellSelectedRootSpaceId.set("");
			shellCurrentUserId.set("");
			shellRecoveryState.set(null);
			shellPickerCustomEmoji.set([]);
			window.location.replace("/");
		} catch (error) {
			shellErrorMessage.set(error instanceof Error ? error.message : "Failed to log out");
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
		<h3 class="h6">Media Storage</h3>
		<p class="text-xs text-surface-700-300">
			Use in-memory image handling with the app protocol by default. Enable asset storage to use
			disk-backed media cache paths.
		</p>
		<label class="flex items-start gap-2 text-sm text-surface-700-300">
			<input
				type="checkbox"
				checked={useAssetStorage}
				onchange={updateMediaStorageMode}
				disabled={mediaSettingsPending}
			/>
			<span>Use asset storage for images</span>
		</label>

		{#if mediaSettingsMessage}
			<p class="text-xs text-surface-700-300">{mediaSettingsMessage}</p>
		{/if}
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
