<script lang="ts">
	import type { MatrixRoomNotificationMode } from "$lib/settings/types";

	interface Props {
		name: string;
		value: MatrixRoomNotificationMode | null | undefined;
		disabled?: boolean;
		includeDefault?: boolean;
		isDefault?: boolean;
		defaultLabel?: string;
		onchange: (mode: MatrixRoomNotificationMode | null) => void;
	}

	let {
		name,
		value,
		disabled = false,
		includeDefault = false,
		isDefault = false,
		defaultLabel = "Default",
		onchange,
	}: Props = $props();

	const baseOptions: { value: MatrixRoomNotificationMode; label: string }[] = [
		{ value: "allMessages", label: "All messages" },
		{ value: "mentionsAndKeywordsOnly", label: "Mentions and keywords only" },
		{ value: "mute", label: "Mute" },
	];

	const options = $derived<{ value: MatrixRoomNotificationMode | null; label: string }[]>(
		includeDefault ? [{ value: null, label: defaultLabel }, ...baseOptions] : baseOptions,
	);

	function isOptionChecked(optValue: MatrixRoomNotificationMode | null): boolean {
		if (optValue === null) {
			return isDefault || value === null;
		}
		return !isDefault && value === optValue;
	}
</script>

<div class="flex flex-col gap-1">
	{#each options as option (option.value ?? "default")}
		<label
			class="flex items-center gap-2 rounded p-1.5 text-sm cursor-pointer hover:bg-surface-200-800"
			class:opacity-50={disabled}
			class:cursor-not-allowed={disabled}
		>
			<input
				type="radio"
				{name}
				value={option.value ?? "default"}
				checked={isOptionChecked(option.value)}
				onchange={() => onchange(option.value)}
				{disabled}
			/>
			<span>{option.label}</span>
		</label>
	{/each}
</div>
