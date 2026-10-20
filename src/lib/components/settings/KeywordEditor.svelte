<script lang="ts">
	interface Props {
		keywords: string[];
		disabled?: boolean;
		emptyMessage?: string;
		placeholder?: string;
		buttonPreset?: string;
		onadd: (keyword: string) => Promise<unknown> | unknown;
		onremove: (keyword: string) => Promise<unknown> | unknown;
	}

	let {
		keywords,
		disabled = false,
		emptyMessage = "No keywords yet.",
		placeholder = "Add a keyword",
		buttonPreset = "preset-tonal",
		onadd,
		onremove,
	}: Props = $props();

	let newKeyword = $state("");
	let submitting = $state(false);

	async function handleAdd(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = newKeyword.trim();
		if (!trimmed || disabled || submitting) {
			return;
		}

		submitting = true;
		try {
			await onadd(trimmed);
			newKeyword = "";
		} finally {
			submitting = false;
		}
	}
</script>

<div class="space-y-2">
	{#if keywords.length > 0}
		<ul class="flex flex-wrap gap-1.5">
			{#each keywords as keyword (keyword)}
				<li class="flex items-center gap-1 rounded-full bg-surface-200-800 px-2.5 py-1 text-xs">
					<span>{keyword}</span>
					<button
						type="button"
						class="hover:text-error-500 transition-colors"
						aria-label={`Remove keyword ${keyword}`}
						onclick={() => onremove(keyword)}
						disabled={disabled}
					>
						✕
					</button>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="text-sm text-surface-700-300">{emptyMessage}</p>
	{/if}

	<form onsubmit={handleAdd} class="flex gap-2">
		<input
			type="text"
			{placeholder}
			bind:value={newKeyword}
			class="input preset-outlined-surface-200-800 bg-surface-50-950 px-2 py-1 text-sm flex-1 min-w-0"
			disabled={disabled || submitting}
			aria-label={placeholder}
		/>
		<button
			type="submit"
			class="btn {buttonPreset} px-3 py-1 text-sm"
			disabled={disabled || submitting || !newKeyword.trim()}
		>
			Add
		</button>
	</form>
</div>
