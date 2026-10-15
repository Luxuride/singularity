<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
import {
		matrixRoomAddKeyword,
		matrixRoomGetKeywords,
		matrixRoomGetSettings,
		matrixRoomLeave,
		matrixRoomRemoveKeyword,
		matrixRoomRemoveTag,
		matrixRoomSetFlags,
		matrixRoomSetNotificationMode,
		matrixRoomSetTag,
} from "$lib/settings/api";
import type {
		MatrixRoomNotificationMode,
		MatrixRoomSettingsResponse,
		MatrixRoomTag,
} from "$lib/settings/types";
	import { shellErrorMessage, shellSelectedRoomId } from "$lib/chats/shell";

	interface Props {
		open: boolean;
		roomId: string;
		roomName: string;
		isSpace: boolean;
		onClose: () => void;
	}

	let { open, roomId, roomName, isSpace, onClose }: Props = $props();

	let dialogElement: HTMLDialogElement | undefined = $state();
	let settings = $state<MatrixRoomSettingsResponse | null>(null);
	let loading = $state(false);
	let saving = $state(false);
	let errorMessage = $state("");
	let newTagName = $state("");
	let confirmingLeave = $state(false);
	let activeTab = $state<"notifications" | "keywords" | "tags" | "danger">("notifications");
	let roomKeywords = $state<string[]>([]);

	const subjectLabel = $derived(isSpace ? "space" : "room");

	const notificationModeOptions: { value: MatrixRoomNotificationMode; label: string }[] = [
		{ value: "allMessages", label: "All messages" },
		{ value: "mentionsAndKeywordsOnly", label: "Mentions and keywords only" },
		{ value: "mute", label: "Mute" },
	];

	$effect(() => {
		if (open && dialogElement) {
			if (!dialogElement.open) {
				dialogElement.showModal();
				void loadSettings();
			}
		} else if (!open && dialogElement && dialogElement.open) {
			dialogElement.close();
		}
	});

	function resetState() {
		settings = null;
		errorMessage = "";
		newTagName = "";
		confirmingLeave = false;
		roomKeywords = [];
		activeTab = "notifications";
	}

	async function loadSettings() {
		resetState();
		loading = true;

		try {
			[settings, roomKeywords] = await Promise.all([
				matrixRoomGetSettings({ roomId }),
				matrixRoomGetKeywords({ roomId }).then((response) => response.keywords),
			]);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to load settings";
		} finally {
			loading = false;
		}
	}

	function handleClose() {
		if (saving) {
			return;
		}
		resetState();
		onClose();
	}

	async function applyChange(action: () => Promise<unknown>) {
		saving = true;
		errorMessage = "";

		try {
			await action();
			settings = await matrixRoomGetSettings({ roomId });
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to update settings";
		} finally {
			saving = false;
		}
	}

	function setNotificationMode(mode: MatrixRoomNotificationMode | null) {
		if (!settings || mode === settings.notificationMode) {
			return;
		}
		void applyChange(() => matrixRoomSetNotificationMode({ roomId, mode }));
	}

	function setFlags(isFavourite: boolean, isLowPriority: boolean) {
		void applyChange(() => matrixRoomSetFlags({ roomId, isFavourite, isLowPriority }));
	}

	function removeTag(tag: MatrixRoomTag) {
		void applyChange(() => matrixRoomRemoveTag({ roomId, name: tag.name }));
	}

	async function addRoomKeyword(event: SubmitEvent) {
		event.preventDefault();
		const keyword = newTagName.trim();
		if (!keyword || saving) {
			return;
		}
		await applyChange(() => matrixRoomAddKeyword({ roomId, keyword }).then((response) => {
			roomKeywords = response.keywords;
		}));
		newTagName = "";
	}

	function removeRoomKeyword(keyword: string) {
		void applyChange(() =>
			matrixRoomRemoveKeyword({ roomId, keyword }).then((response) => {
				roomKeywords = response.keywords;
			}),
		);
	}

	async function addTag(event: SubmitEvent) {
		event.preventDefault();
		const name = newTagName.trim();
		if (!name || saving) {
			return;
		}

		await applyChange(() => matrixRoomSetTag({ roomId, name, order: null }));
		newTagName = "";
	}

	async function leave() {
		if (!settings?.joined || saving) {
			return;
		}

		saving = true;
		errorMessage = "";

		try {
			await matrixRoomLeave({ roomId, forget: true });
			confirmingLeave = false;
			resetState();
			onClose();

			if ($shellSelectedRoomId === roomId) {
				shellSelectedRoomId.set("");
			}
			shellErrorMessage.set("");

			const params = new URLSearchParams(page.url.searchParams);
			params.delete("roomId");
			const search = params.toString();
			await goto(search ? `/chats?${search}` : "/chats", { replaceState: true });
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : `Failed to leave ${subjectLabel}`;
		} finally {
			saving = false;
		}
	}
</script>

<dialog
	bind:this={dialogElement}
	onclose={handleClose}
	class="backdrop:bg-surface-900/50 card preset-outlined-surface-200-800 bg-surface-100-900 m-auto rounded-xl p-4 shadow-xl min-w-[380px] max-w-[480px] w-full max-h-[85vh] overflow-y-auto"
	onmousedown={(event) => {
		if (event.target === dialogElement) handleClose();
	}}
>
	<div class="flex flex-col gap-4">
		<header class="flex items-start justify-between gap-2">
			<div class="min-w-0">
				<p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Settings</p>
				<h2 class="h5 truncate">{isSpace ? "Space" : "Room"} settings</h2>
				<p class="text-xs text-surface-700-300 truncate">{roomName}</p>
			</div>
			<button
				type="button"
				class="btn preset-tonal px-2 py-1 text-sm"
				onclick={handleClose}
				disabled={saving}
				aria-label="Close settings"
			>
				✕
			</button>
		</header>

		<nav class="card p-2 preset-outlined-surface-200-800 bg-surface-50-950">
			<ul class="flex flex-wrap gap-2">
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "notifications"}
						class:preset-tonal={activeTab !== "notifications"}
						onclick={() => (activeTab = "notifications")}
					>
						Notifications
					</button>
				</li>
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "keywords"}
						class:preset-tonal={activeTab !== "keywords"}
						onclick={() => (activeTab = "keywords")}
					>
						Keywords
					</button>
				</li>
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "tags"}
						class:preset-tonal={activeTab !== "tags"}
						onclick={() => (activeTab = "tags")}
					>
						Tags
					</button>
				</li>
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "danger"}
						class:preset-tonal={activeTab !== "danger"}
						onclick={() => (activeTab = "danger")}
					>
						Danger Zone
					</button>
				</li>
			</ul>
		</nav>

		{#if loading}
			<p class="text-sm text-surface-700-300">Loading settings...</p>
		{:else if settings}
			{#if errorMessage}
				<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
			{/if}

			{#if activeTab === "notifications"}
			<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
				<h3 class="h6">Notifications</h3>
				<div class="flex flex-col gap-1">
					{#each notificationModeOptions as option (option.value)}
						<label
							class="flex items-center gap-2 rounded p-1.5 text-sm cursor-pointer hover:bg-surface-200-800"
						>
							<input
								type="radio"
								name="notification-mode"
								value={option.value}
								checked={settings.notificationMode === option.value}
								onchange={() => setNotificationMode(option.value)}
								disabled={saving || !settings.joined}
							/>
							<span>{option.label}</span>
						</label>
					{/each}
				</div>
				{#if settings.joined}
					<button
						type="button"
						class="text-xs text-surface-700-300 underline text-left hover:text-surface-900-100"
						onclick={() => setNotificationMode(null)}
						disabled={saving || !settings.notificationModeUserDefined}
					>
						Reset to default
					</button>
				{/if}
			</section>
			{/if}

			{#if activeTab === "keywords"}
			<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
				<h3 class="h6">Keywords</h3>
				<p class="text-xs text-surface-700-300">
					Notify for messages in this {subjectLabel} containing a keyword, even when the
					notification mode is "Mentions and keywords only" or "Mute".
				</p>
				{#if roomKeywords.length > 0}
					<ul class="flex flex-wrap gap-1">
						{#each roomKeywords as keyword (keyword)}
							<li class="flex items-center gap-1 rounded-full bg-surface-200-800 px-2 py-1 text-xs">
								<span>{keyword}</span>
								<button
									type="button"
									class="hover:text-error-500"
									aria-label={`Remove keyword ${keyword}`}
									onclick={() => removeRoomKeyword(keyword)}
									disabled={saving}
								>
									✕
								</button>
							</li>
						{/each}
					</ul>
				{:else}
					<p class="text-sm text-surface-700-300">No keywords for this {subjectLabel} yet.</p>
				{/if}

				<form onsubmit={addRoomKeyword} class="flex gap-2">
					<input
						type="text"
						placeholder="Add a keyword"
						bind:value={newTagName}
						class="input preset-outlined-surface-200-800 bg-surface-50-950 px-2 py-1 text-sm flex-1 min-w-0"
						disabled={saving}
						aria-label="New keyword"
					/>
					<button
						type="submit"
						class="btn preset-tonal px-3 py-1 text-sm"
						disabled={saving || !newTagName.trim()}
					>
						Add
					</button>
				</form>
			</section>
			{/if}

			{#if activeTab === "tags"}
			<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
				<h3 class="h6">Tags</h3>
				<div class="flex flex-col gap-1">
					<label class="flex items-center gap-2 rounded p-1.5 text-sm cursor-pointer hover:bg-surface-200-800">
						<input
							type="checkbox"
							checked={settings.isFavourite}
							onchange={(event) =>
								setFlags(event.currentTarget.checked, settings?.isLowPriority ?? false)}
							disabled={saving}
						/>
						<span>Favourite</span>
					</label>
					<label class="flex items-center gap-2 rounded p-1.5 text-sm cursor-pointer hover:bg-surface-200-800">
						<input
							type="checkbox"
							checked={settings.isLowPriority}
							onchange={(event) =>
								setFlags(settings?.isFavourite ?? false, event.currentTarget.checked)}
							disabled={saving}
						/>
						<span>Low priority</span>
					</label>
				</div>

				{#if settings.tags.filter((tag) => tag.name !== "m.favourite" && tag.name !== "m.lowpriority").length > 0}
					<ul class="flex flex-wrap gap-1">
						{#each settings.tags.filter((tag) => tag.name !== "m.favourite" && tag.name !== "m.lowpriority") as tag (tag.name)}
							<li class="flex items-center gap-1 rounded-full bg-surface-200-800 px-2 py-1 text-xs">
								<span>{tag.name}</span>
								<button
									type="button"
									class="hover:text-error-500"
									aria-label={`Remove tag ${tag.name}`}
									onclick={() => removeTag(tag)}
									disabled={saving}
								>
									✕
								</button>
							</li>
						{/each}
					</ul>
				{/if}

				<form onsubmit={addTag} class="flex gap-2">
					<input
						type="text"
						placeholder="u.tag-name"
						bind:value={newTagName}
						class="input preset-outlined-surface-200-800 bg-surface-50-950 px-2 py-1 text-sm flex-1 min-w-0"
						disabled={saving}
						aria-label="New tag name"
					/>
					<button
						type="submit"
						class="btn preset-tonal px-3 py-1 text-sm"
						disabled={saving || !newTagName.trim()}
					>
						Add
					</button>
				</form>
			</section>
			{/if}

			{#if activeTab === "danger"}
			{#if settings.joined}
				<section class="card p-3 preset-outlined-error-500 bg-surface-50-950 space-y-2">
					<h3 class="h6 text-error-500">Danger zone</h3>
					{#if confirmingLeave}
						<p class="text-sm text-surface-700-300">
							Leave and forget this {subjectLabel}? You will need a new invite to rejoin.
						</p>
						<div class="flex gap-2">
							<button
								type="button"
								class="btn preset-tonal px-3 py-1 text-sm"
								onclick={() => (confirmingLeave = false)}
								disabled={saving}
							>
								Cancel
							</button>
							<button
								type="button"
								class="btn preset-filled-error-500 px-3 py-1 text-sm"
								onclick={leave}
								disabled={saving}
							>
								{saving ? "Leaving..." : `Leave ${subjectLabel}`}
							</button>
						</div>
					{:else}
						<button
							type="button"
							class="btn preset-outlined-error-500 px-3 py-1 text-sm self-start"
							onclick={() => (confirmingLeave = true)}
							disabled={saving}
						>
							Leave {subjectLabel}
						</button>
					{/if}
				</section>
			{:else}
				<p class="text-sm text-surface-700-300">
					Not joined — notification settings and leaving are unavailable.
				</p>
			{/if}
			{/if}
		{:else if errorMessage}
			<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
		{/if}
	</div>
</dialog>
