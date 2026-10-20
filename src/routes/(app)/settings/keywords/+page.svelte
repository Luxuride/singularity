<script lang="ts">
	import { goto } from "$app/navigation";
	import { onMount } from "svelte";

	onMount(() => {
		void goto("/settings/notifications", { replaceState: true });
	});
</script>

<p class="text-sm text-surface-700-300">Redirecting to Notifications...</p>
