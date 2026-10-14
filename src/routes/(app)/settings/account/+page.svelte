<script lang="ts">
	import { matrixChangePassword } from "$lib/settings/api";

	let changing = $state(false);
	let accountMessage = $state("");
	let accountError = $state("");

	let newPassword = $state("");
	let confirmPassword = $state("");

	async function changePassword() {
		if (!newPassword) {
			accountError = "New password is required.";
			return;
		}

		if (newPassword !== confirmPassword) {
			accountError = "Passwords do not match.";
			return;
		}

		changing = true;
		accountMessage = "";
		accountError = "";

		try {
			await matrixChangePassword({ newPassword });
			accountMessage = "Password changed.";
			newPassword = "";
			confirmPassword = "";
		} catch (error) {
			accountError = error instanceof Error ? error.message : "Failed to change password";
		} finally {
			changing = false;
		}
	}
</script>

<section class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 space-y-4 max-h-[70vh] overflow-y-auto">
	<header>
		<p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Settings</p>
		<h2 class="h5">Account</h2>
		<p class="text-xs text-surface-700-300">Password and account management.</p>
	</header>

	{#if accountError}
		<p class="card p-2 text-sm preset-filled-error-500">{accountError}</p>
	{/if}

	{#if accountMessage}
		<p class="text-xs text-surface-700-300">{accountMessage}</p>
	{/if}

	<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-3">
		<h3 class="h6">Change password</h3>
		<p class="text-xs text-surface-700-300">
			Changing the password signs out other devices where the old password is still active.
		</p>
		<div class="space-y-2 max-w-sm">
			<input
				type="password"
				class="input preset-outlined-surface-300-700 bg-surface-50-950 w-full"
				bind:value={newPassword}
				placeholder="New password"
				autocomplete="new-password"
			/>
			<input
				type="password"
				class="input preset-outlined-surface-300-700 bg-surface-50-950 w-full"
				bind:value={confirmPassword}
				placeholder="Confirm new password"
				autocomplete="new-password"
				onkeydown={(event) => {
					if (event.key === "Enter") {
						event.preventDefault();
						void changePassword();
					}
				}}
			/>
			<button
				type="button"
				class="btn preset-tonal"
				onclick={changePassword}
				disabled={changing}
			>
				{#if changing}Changing...{:else}Change password{/if}
			</button>
		</div>
	</section>
</section>
