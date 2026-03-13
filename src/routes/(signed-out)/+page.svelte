<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    matrixCompleteOAuth,
    matrixSessionStatus,
    matrixStartOAuth,
  } from "../../lib/auth/api";

  let homeserverUrl = $state("https://matrix.org");
  let callbackUrl = $state("");
  let authorizationUrl = $state("");
  let redirectUri = $state("");

  let loadingSession = $state(true);
  let startingOAuth = $state(false);
  let completingOAuth = $state(false);

  let errorMessage = $state("");
  let infoMessage = $state("");

  let authenticated = $state(false);

  onMount(async () => {
    await refreshSession();
  });

  async function refreshSession() {
    loadingSession = true;
    errorMessage = "";

    try {
      const status = await matrixSessionStatus();
      authenticated = status.authenticated;

      if (status.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load session";
    } finally {
      loadingSession = false;
    }
  }

  async function startOAuthLogin(event: Event) {
    event.preventDefault();
    startingOAuth = true;
    errorMessage = "";
    infoMessage = "";

    try {
      const result = await matrixStartOAuth({ homeserverUrl });
      authorizationUrl = result.authorizationUrl;
      redirectUri = result.redirectUri;

      await openUrl(result.authorizationUrl);
      infoMessage = "Browser opened. Complete sign-in and paste the callback URL below.";
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to start OAuth login";
    } finally {
      startingOAuth = false;
    }
  }

  async function completeOAuthLogin(event: Event) {
    event.preventDefault();
    completingOAuth = true;
    errorMessage = "";
    infoMessage = "";

    try {
      const response = await matrixCompleteOAuth({ callbackUrl });
      authenticated = response.authenticated;
      callbackUrl = "";
      authorizationUrl = "";
      infoMessage = "Session created successfully.";

      if (response.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to complete OAuth login";
    } finally {
      completingOAuth = false;
    }
  }

</script>

<main class="min-h-screen p-4 md:p-8 grid place-items-center">
  <section class="card w-full max-w-5xl p-4 md:p-6 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950">
    <header class="space-y-1">
      <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Singularity</p>
      <h1 class="h2">Matrix OAuth2 Login</h1>
      <p class="text-surface-700-300">Desktop sign-in with browser-based Matrix SSO and callback completion.</p>
    </header>

    {#if loadingSession}
      <p class="card p-3 text-sm bg-surface-100-900">Loading session...</p>
    {:else if authenticated}
      <p class="card p-3 text-sm bg-surface-100-900">Session active. Redirecting to chats...</p>
    {:else}
      <div class="grid gap-4 lg:grid-cols-2">
        <form class="card p-4 space-y-3 preset-outlined-surface-200-800 bg-surface-100-900" onsubmit={startOAuthLogin}>
          <h2 class="h4">Start Login</h2>
          <label class="label" for="homeserver">Homeserver URL</label>
        <input
          class="input"
          id="homeserver"
          type="url"
          bind:value={homeserverUrl}
          placeholder="https://matrix.org"
          required
        />

          <button class="btn preset-filled-primary-500" type="submit" disabled={startingOAuth}>
            {#if startingOAuth}Starting...{:else}Start Matrix OAuth2{/if}
          </button>
        </form>

        <form class="card p-4 space-y-3 preset-outlined-surface-200-800 bg-surface-100-900" onsubmit={completeOAuthLogin}>
          <h2 class="h4">Complete Login</h2>
          <p class="text-sm text-surface-700-300">
            After browser sign-in, copy the full callback URL from your browser address bar and paste it below.
          </p>

          {#if redirectUri}
            <p class="text-sm text-surface-700-300">Expected redirect URI: <strong>{redirectUri}</strong></p>
          {/if}

          {#if authorizationUrl}
            <p class="text-sm text-surface-700-300">Opened authorization URL: <strong>{authorizationUrl}</strong></p>
          {/if}

          <label class="label" for="callback">Callback URL</label>
          <textarea
            class="textarea"
            id="callback"
            bind:value={callbackUrl}
            placeholder="http://127.0.0.1:8743/matrix-oauth-callback?loginToken=..."
            rows="4"
            required
          ></textarea>

          <button class="btn preset-filled-primary-500" type="submit" disabled={completingOAuth}>
            {#if completingOAuth}Completing...{:else}Complete Login{/if}
          </button>
        </form>
      </div>
    {/if}

    {#if errorMessage}
      <p class="card p-3 text-sm preset-filled-error-500">{errorMessage}</p>
    {/if}

    {#if infoMessage}
      <p class="card p-3 text-sm preset-filled-success-500">{infoMessage}</p>
    {/if}
  </section>
</main>
