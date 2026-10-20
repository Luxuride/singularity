<script lang="ts">
	import { onMount } from "svelte";

	import {
		matrixAddKeyword,
		matrixGetKeywords,
		matrixGetNotificationDefaults,
		matrixRemoveKeyword,
		matrixSetNotificationDefault,
	} from "$lib/settings/api";
	import type {
		MatrixNotificationDefaultsResponse,
		MatrixRoomNotificationMode,
	} from "$lib/settings/types";
	import { KeywordEditor, NotificationModeSelector } from "$lib/components/settings";

	let loading = $state(true);
	let savingKeys = $state<Record<string, boolean>>({});
	let savingKeyword = $state(false);
	let defaults = $state<MatrixNotificationDefaultsResponse | null>(null);
	let keywords = $state<string[]>([]);
	let errorMessage = $state("");

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
		void loadNotificationSettings();
	});

	async function loadNotificationSettings() {
		loading = true;
		errorMessage = "";

		try {
			const [defaultsRes, keywordsRes] = await Promise.all([
				matrixGetNotificationDefaults(),
				matrixGetKeywords(),
			]);
			defaults = defaultsRes;
			keywords = keywordsRes.keywords;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to load notification settings";
		} finally {
			loading = false;
		}
	}

	async function setDefault(
		key: (typeof defaultSections)[number]["key"],
		mode: MatrixRoomNotificationMode | null,
	) {
		if (!defaults || !mode || savingKeys[key]) {
			return;
		}
		if (defaults[key] === mode) {
			return;
		}

		const previous = defaults[key];
		defaults[key] = mode;
		savingKeys[key] = true;
		errorMessage = "";

		try {
			await matrixSetNotificationDefault({
				encrypted: key === "encrypted" || key === "encryptedOneToOne",
				oneToOne: key === "unencryptedOneToOne" || key === "encryptedOneToOne",
				mode,
			});
		} catch (error) {
			defaults[key] = previous;
			errorMessage = error instanceof Error ? error.message : "Failed to update notification default";
		} finally {
			savingKeys[key] = false;
		}
	}

	async function addKeyword(keyword: string) {
		if (savingKeyword) {
			return;
		}

		savingKeyword = true;
		errorMessage = "";

		try {
			const response = await matrixAddKeyword({ keyword });
			keywords = response.keywords;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to add keyword";
		} finally {
			savingKeyword = false;
		}
	}

	async function removeKeyword(keyword: string) {
		if (savingKeyword) {
			return;
		}

		savingKeyword = true;
		errorMessage = "";

		try {
			const response = await matrixRemoveKeyword({ keyword });
			keywords = response.keywords;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to remove keyword";
		} finally {
			savingKeyword = false;
		}
	}
</script>

<section class="space-y-4">
	<header>
		<h1 class="h4">Notifications</h1>
		<p class="text-sm text-surface-700-300">
			Default notification modes per room type and global notification keywords. Individual rooms and
			spaces can override these in their own settings.
		</p>
	</header>

	{#if loading}
		<p class="text-sm text-surface-700-300">Loading notification settings...</p>
	{:else}
		{#if errorMessage}
			<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
		{/if}

		<div class="flex flex-col gap-6 max-w-xl">
			{#if defaults}
				<section class="space-y-3">
					<div>
						<h2 class="h5">Room defaults</h2>
						<p class="text-xs text-surface-700-300">
							Default notification mode applied to rooms without custom rules.
						</p>
					</div>

					<div class="flex flex-col gap-3">
						{#each defaultSections as section (section.key)}
							<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
								<div>
									<h3 class="h6">{section.label}</h3>
									<p class="text-xs text-surface-700-300">{section.description}</p>
								</div>
								<NotificationModeSelector
									name={`notification-default-${section.key}`}
									value={defaults[section.key]}
									onchange={(mode) => setDefault(section.key, mode)}
								/>
							</section>
						{/each}
					</div>
				</section>
			{/if}

			<section class="space-y-3">
				<div>
					<h2 class="h5">Keywords</h2>
					<p class="text-xs text-surface-700-300">
						Rooms set to "Mentions and keywords only" also notify you when one of these keywords appears
						in a message.
					</p>
				</div>

				<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
					<KeywordEditor
						{keywords}
						disabled={savingKeyword}
						buttonPreset="preset-filled-primary-500"
						onadd={addKeyword}
						onremove={removeKeyword}
					/>
				</section>
			</section>
		</div>
	{/if}
</section>
