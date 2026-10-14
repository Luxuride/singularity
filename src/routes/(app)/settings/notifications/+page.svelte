<script lang="ts">
	import { onMount } from "svelte";

	import {
		matrixGetNotificationDefaults,
		matrixSetNotificationDefault,
	} from "$lib/settings/api";
	import type {
		MatrixNotificationDefaultsResponse,
		MatrixRoomNotificationMode,
	} from "$lib/settings/types";

	let loading = $state(true);
	let savingKey = $state("");
	let defaults = $state<MatrixNotificationDefaultsResponse | null>(null);
	let errorMessage = $state("");

	const modeOptions: { value: MatrixRoomNotificationMode; label: string }[] = [
		{ value: "allMessages", label: "All messages" },
		{ value: "mentionsAndKeywordsOnly", label: "Mentions and keywords only" },
		{ value: "mute", label: "Mute" },
	];

	const defaultSections: {
		key: "encrypted" | "encryptedOneToOne" | "unencrypted" | "unencryptedOneToOne";
		label: string;
		description: string;
	}[] = [
		{
			key: "unencrypted",
			label: "Rooms",
			description: "Group rooms that are not encrypted.",
		},
		{
			key: "unencryptedOneToOne",
			label: "Direct chats",
			description: "One-to-one rooms with exactly two members, not encrypted.",
		},
		{
			key: "encrypted",
			label: "Encrypted rooms",
			description: "Group rooms with encryption enabled.",
		},
		{
			key: "encryptedOneToOne",
			label: "Encrypted direct chats",
			description: "One-to-one rooms with exactly two members and encryption enabled.",
		},
	];

	onMount(() => {
		void loadDefaults();
	});

	async function loadDefaults() {
		loading = true;
		errorMessage = "";

		try {
			defaults = await matrixGetNotificationDefaults();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to load notification defaults";
		} finally {
			loading = false;
		}
	}

	async function setDefault(
		key: (typeof defaultSections)[number]["key"],
		mode: MatrixRoomNotificationMode,
	) {
		if (!defaults || savingKey) {
			return;
		}
		if (defaults[key] === mode) {
			return;
		}

		savingKey = key;
		errorMessage = "";

		try {
			await matrixSetNotificationDefault({
				encrypted: key === "encrypted" || key === "encryptedOneToOne",
				oneToOne: key === "unencryptedOneToOne" || key === "encryptedOneToOne",
				mode,
			});
			defaults = await matrixGetNotificationDefaults();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to update notification default";
			await loadDefaults();
		} finally {
			savingKey = "";
		}
	}
</script>

<section class="space-y-4">
	<header>
		<h1 class="h4">Notifications</h1>
		<p class="text-sm text-surface-700-300">
			Default notification modes per room type. Individual rooms and spaces can override these in
			their own settings.
		</p>
	</header>

	{#if loading}
		<p class="text-sm text-surface-700-300">Loading notification defaults...</p>
	{:else if defaults}
		{#if errorMessage}
			<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
		{/if}

		<div class="flex flex-col gap-3 max-w-xl">
			{#each defaultSections as section (section.key)}
				<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
					<div>
						<h2 class="h6">{section.label}</h2>
						<p class="text-xs text-surface-700-300">{section.description}</p>
					</div>
					<div class="flex flex-wrap gap-2">
						{#each modeOptions as option (option.value)}
							<button
								type="button"
								class="btn text-sm"
								class:preset-filled-primary-500={defaults[section.key] === option.value}
								class:preset-tonal={defaults[section.key] !== option.value}
								onclick={() => setDefault(section.key, option.value)}
								disabled={savingKey === section.key}
							>
								{option.label}
							</button>
						{/each}
					</div>
				</section>
			{/each}
		</div>
	{/if}
</section>
