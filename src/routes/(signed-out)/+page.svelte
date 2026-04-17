<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    matrixCompleteOAuth,
    matrixSessionStatus,
    matrixStartOAuth,
  } from "../../lib/auth/api";

  let homeserverUrl = $state("https://matrix.org");

  let loadingSession = $state(true);
  let startingOAuth = $state(false);
  let completingOAuth = $state(false);
  let waitingForCallback = $state(false);

  let errorMessage = $state("");
  let infoMessage = $state("");

  let authenticated = $state(false);
  let lastHandledCallbackUrl = "";

  onMount(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      await refreshSession();

      try {
        const currentUrls = await getCurrent();
        if (!cancelled) {
          await completeOAuthFromDeepLinks(currentUrls ?? []);
        }

        unlisten = await onOpenUrl((urls) => {
          if (cancelled) {
            return;
          }

          void completeOAuthFromDeepLinks(urls);
        });
      } catch (error) {
        errorMessage = error instanceof Error ? error.message : "Failed to initialize deep-link listener";
      }
    })();

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  });

  function findOAuthCallbackUrl(urls: string[]): string | null {
    for (const urlString of urls) {
      try {
        const parsed = new URL(urlString);
        if (parsed.protocol !== "singularity:" || parsed.hostname !== "oauth-callback") {
          continue;
        }

        return urlString;
      } catch {
        continue;
      }
    }

    return null;
  }

  async function completeOAuthFromDeepLinks(urls: string[]) {
    const callbackUrl = findOAuthCallbackUrl(urls);

    if (!callbackUrl || completingOAuth || callbackUrl === lastHandledCallbackUrl) {
      return;
    }

    await completeOAuthLogin(callbackUrl);
  }

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
    waitingForCallback = false;
    errorMessage = "";
    infoMessage = "";

    try {
      const result = await matrixStartOAuth({ homeserverUrl });
      waitingForCallback = true;
      lastHandledCallbackUrl = "";

      await openUrl(result.authorizationUrl);
      infoMessage = "Browser opened. Complete sign-in to continue.";
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to start OAuth login";
    } finally {
      startingOAuth = false;
    }
  }

  async function completeOAuthLogin(callbackUrl: string) {
    completingOAuth = true;
    errorMessage = "";

    try {
      const response = await matrixCompleteOAuth({ callbackUrl });
      lastHandledCallbackUrl = callbackUrl;
      authenticated = response.authenticated;
      waitingForCallback = false;
      infoMessage = "Signed in successfully.";

      if (response.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      waitingForCallback = false;
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

        <button class="btn preset-filled-primary-500" type="submit" disabled={startingOAuth || completingOAuth}>
          {#if startingOAuth}
            Starting...
          {:else if completingOAuth}
            Completing Login...
          {:else if waitingForCallback}
            Waiting for Browser Sign-In...
          {:else}
            Start Matrix OAuth2
          {/if}
        </button>

        <p class="text-sm text-surface-700-300">
          Sign-in completes automatically after browser authentication. No callback URL copy and paste is required.
        </p>

        {#if waitingForCallback}
          <p class="text-sm text-surface-700-300">Waiting for browser callback. You can restart sign-in at any time.</p>
        {/if}
      </form>
    {/if}

    {#if errorMessage}
      <p class="card p-3 text-sm preset-filled-error-500">{errorMessage}</p>
    {/if}

    {#if infoMessage}
      <p class="card p-3 text-sm preset-filled-success-500">{infoMessage}</p>
    {/if}
  </section>
</main>
