<script lang="ts">
	import { onMount } from "svelte";

	import { open } from "@tauri-apps/plugin-dialog";

	import { matrixGetProfile, matrixRemoveAvatar, matrixSetAvatar, matrixSetDisplayName } from "$lib/settings/api";

	let loading = $state(true);
	let savingName = $state(false);
	let savingAvatar = $state(false);
	let profileMessage = $state("");
	let profileError = $state("");

	let userId = $state("");
	let displayName = $state("");
	let displayNameInput = $state("");
	let avatarUrl = $state<string | null>(null);

	onMount(() => {
		void loadProfile();
	});

	async function loadProfile() {
		loading = true;
		profileError = "";

		try {
			const profile = await matrixGetProfile();
			userId = profile.userId;
			displayName = profile.displayName ?? "";
			displayNameInput = displayName;
			avatarUrl = profile.avatarUrl ?? null;
		} catch (error) {
			profileError = error instanceof Error ? error.message : "Failed to load profile";
		} finally {
			loading = false;
		}
	}

	async function saveDisplayName() {
		const next = displayNameInput.trim();
		if (next === displayName.trim()) {
			return;
		}

		savingName = true;
		profileMessage = "";
		profileError = "";

		try {
			await matrixSetDisplayName({ displayName: next || null });
			displayName = next;
			profileMessage = "Display name updated.";
		} catch (error) {
			profileError = error instanceof Error ? error.message : "Failed to update display name";
		} finally {
			savingName = false;
		}
	}

	async function chooseAvatar() {
		const selected = await open({
			multiple: false,
			directory: false,
			title: "Choose an avatar image",
		});

		if (typeof selected !== "string" || selected.length === 0) {
			return;
		}

		savingAvatar = true;
		profileMessage = "";
		profileError = "";

		try {
			await matrixSetAvatar({ filePath: selected });
			await loadProfile();
			profileMessage = "Avatar updated.";
		} catch (error) {
			profileError = error instanceof Error ? error.message : "Failed to update avatar";
		} finally {
			savingAvatar = false;
		}
	}

	async function removeAvatar() {
		savingAvatar = true;
		profileMessage = "";
		profileError = "";

		try {
			await matrixRemoveAvatar();
			avatarUrl = null;
			profileMessage = "Avatar removed.";
		} catch (error) {
			profileError = error instanceof Error ? error.message : "Failed to remove avatar";
		} finally {
			savingAvatar = false;
		}
	}

	const avatarLabel = $derived.by(() => {
		const localPart = userId.includes(":") ? userId.slice(0, userId.indexOf(":")) : userId;
		return localPart.replace(/^@/, "").charAt(0).toUpperCase() || "#";
	});
</script>

<section class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 space-y-4 max-h-[70vh] overflow-y-auto">
	<header>
		<p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Settings</p>
		<h2 class="h5">Profile</h2>
		<p class="text-xs text-surface-700-300">{userId}</p>
	</header>

	{#if profileError}
		<p class="card p-2 text-sm preset-filled-error-500">{profileError}</p>
	{/if}

	{#if profileMessage}
		<p class="text-xs text-surface-700-300">{profileMessage}</p>
	{/if}

	{#if loading}
		<p class="text-sm text-surface-700-300">Loading profile...</p>
	{:else}
		<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-3">
			<h3 class="h6">Avatar</h3>
			<div class="flex items-center gap-4">
				<div class="h-16 w-16 shrink-0 overflow-hidden rounded-full bg-surface-200-800 grid place-items-center text-xl font-semibold text-surface-800-200">
					{#if avatarUrl}
						<img src={avatarUrl} alt="Avatar" class="h-full w-full object-cover" />
					{:else}
						<span>{avatarLabel}</span>
					{/if}
				</div>
				<div class="flex flex-wrap gap-2">
					<button
						type="button"
						class="btn preset-tonal text-sm"
						onclick={chooseAvatar}
						disabled={savingAvatar}
					>
						{#if savingAvatar}Working...{:else}Choose image...{/if}
					</button>
					{#if avatarUrl}
						<button
							type="button"
							class="btn preset-tonal text-sm"
							onclick={removeAvatar}
							disabled={savingAvatar}
						>
							Remove
						</button>
					{/if}
				</div>
			</div>
		</section>

		<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-3">
			<h3 class="h6">Display name</h3>
			<div class="flex flex-wrap gap-2">
				<input
					type="text"
					class="input preset-outlined-surface-300-700 bg-surface-50-950 flex-1 min-w-48"
					bind:value={displayNameInput}
					placeholder="Display name"
					onkeydown={(event) => {
						if (event.key === "Enter") {
							event.preventDefault();
							void saveDisplayName();
						}
					}}
				/>
				<button
					type="button"
					class="btn preset-tonal text-sm"
					onclick={saveDisplayName}
					disabled={savingName}
				>
					{#if savingName}Saving...{:else}Save{/if}
				</button>
			</div>
		</section>
	{/if}
</section>
