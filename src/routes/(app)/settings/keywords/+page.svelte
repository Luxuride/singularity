<script lang="ts">
	import { onMount } from "svelte";

	import { matrixAddKeyword, matrixGetKeywords, matrixRemoveKeyword } from "$lib/settings/api";

	let loading = $state(true);
	let saving = $state(false);
	let keywords = $state<string[]>([]);
	let newKeyword = $state("");
	let errorMessage = $state("");

	onMount(() => {
		void loadKeywords();
	});

	async function loadKeywords() {
		loading = true;
		errorMessage = "";

		try {
			const response = await matrixGetKeywords();
			keywords = response.keywords;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to load keywords";
		} finally {
			loading = false;
		}
	}

	async function addKeyword(event: SubmitEvent) {
		event.preventDefault();
		const keyword = newKeyword.trim();
		if (!keyword || saving) {
			return;
		}

		saving = true;
		errorMessage = "";

		try {
			const response = await matrixAddKeyword({ keyword });
			keywords = response.keywords;
			newKeyword = "";
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to add keyword";
		} finally {
			saving = false;
		}
	}

	async function removeKeyword(keyword: string) {
		if (saving) {
			return;
		}

		saving = true;
		errorMessage = "";

		try {
			const response = await matrixRemoveKeyword({ keyword });
			keywords = response.keywords;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to remove keyword";
		} finally {
			saving = false;
		}
	}
</script>

<section class="space-y-4">
	<header>
		<h1 class="h4">Keywords</h1>
		<p class="text-sm text-surface-700-300">
			Rooms set to "Mentions and keywords only" also notify you when one of these keywords appears
			in a message.
		</p>
	</header>

	{#if loading}
		<p class="text-sm text-surface-700-300">Loading keywords...</p>
	{:else}
		{#if errorMessage}
			<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
		{/if}

		<form onsubmit={addKeyword} class="flex gap-2 max-w-md">
			<input
				type="text"
				placeholder="Add a keyword"
				bind:value={newKeyword}
				class="input preset-outlined-surface-200-800 bg-surface-50-950 px-3 py-2 text-sm flex-1 min-w-0"
				disabled={saving}
				aria-label="New keyword"
			/>
			<button type="submit" class="btn preset-filled-primary-500 px-4 py-2 text-sm" disabled={saving || !newKeyword.trim()}>
				Add
			</button>
		</form>

		{#if keywords.length > 0}
			<ul class="flex flex-wrap gap-2 max-w-md">
				{#each keywords as keyword (keyword)}
					<li class="flex items-center gap-1 rounded-full bg-surface-200-800 px-3 py-1 text-sm">
						<span>{keyword}</span>
						<button
							type="button"
							class="hover:text-error-500"
							aria-label={`Remove keyword ${keyword}`}
							onclick={() => removeKeyword(keyword)}
							disabled={saving}
						>
							✕
						</button>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="text-sm text-surface-700-300">No keywords yet.</p>
		{/if}
	{/if}
</section>
