<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    matrixCompleteOAuth,
    matrixPasswordLogin,
    matrixSessionStatus,
    matrixStartOAuth,
  } from "../../lib/auth/api";

  type SignInMethod = "oauth" | "password";

  let homeserverUrl = $state("https://matrix.org");
  let signInMethod = $state<SignInMethod>("oauth");
  let username = $state("");
  let password = $state("");

  let loadingSession = $state(true);
  let startingOAuth = $state(false);
  let completingOAuth = $state(false);
  let waitingForCallback = $state(false);
  let signingInWithPassword = $state(false);

  let errorMessage = $state("");
  let infoMessage = $state("");

  let authenticated = $state(false);
  let lastHandledCallbackUrl = "";

  let isDevContainer = $state(false);
  let authorizationUrl = $state("");
  let manualCallbackUrl = $state("");

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
      isDevContainer = result.isDevContainer;
      waitingForCallback = true;
      lastHandledCallbackUrl = "";

      if (isDevContainer) {
        authorizationUrl = result.authorizationUrl;
        infoMessage = "Copy the URL below and open it in your browser to sign in.";
      } else {
        authorizationUrl = "";
        await openUrl(result.authorizationUrl);
        infoMessage = "Browser opened. Complete sign-in to continue.";
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to start OAuth login";
    } finally {
      startingOAuth = false;
    }
  }

  async function startPasswordLogin(event: Event) {
    event.preventDefault();
    signingInWithPassword = true;
    waitingForCallback = false;
    errorMessage = "";
    infoMessage = "";

    try {
      const response = await matrixPasswordLogin({
        homeserverUrl,
        username,
        password,
      });

      authenticated = response.authenticated;
      infoMessage = "Signed in successfully.";
      password = "";

      if (response.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to sign in with password";
    } finally {
      signingInWithPassword = false;
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
      manualCallbackUrl = "";
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

  async function handleManualCallback(event: Event) {
    event.preventDefault();
    if (!manualCallbackUrl.trim()) {
      errorMessage = "Please paste the callback URL from your browser.";
      return;
    }
    await completeOAuthLogin(manualCallbackUrl.trim());
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
      <section class="card p-4 space-y-4 preset-outlined-surface-200-800 bg-surface-100-900">
        <h2 class="h4">Start Login</h2>

        <div class="flex gap-2" role="tablist" aria-label="Sign-in methods">
          <button
            class="btn"
            class:preset-filled-primary-500={signInMethod === "oauth"}
            class:preset-filled-surface-500={signInMethod !== "oauth"}
            type="button"
            role="tab"
            aria-selected={signInMethod === "oauth"}
            onclick={() => {
              signInMethod = "oauth";
              errorMessage = "";
              infoMessage = "";
            }}
          >
            OAuth
          </button>
          <button
            class="btn"
            class:preset-filled-primary-500={signInMethod === "password"}
            class:preset-filled-surface-500={signInMethod !== "password"}
            type="button"
            role="tab"
            aria-selected={signInMethod === "password"}
            onclick={() => {
              signInMethod = "password";
              waitingForCallback = false;
              errorMessage = "";
              infoMessage = "";
            }}
          >
            Password
          </button>
        </div>

        <label class="label" for="homeserver">Homeserver URL</label>
        <input
          class="input"
          id="homeserver"
          type="url"
          bind:value={homeserverUrl}
          placeholder="https://matrix.org"
          required
        />

        {#if signInMethod === "oauth"}
          <form class="space-y-3" onsubmit={startOAuthLogin}>
            <button
              class="btn preset-filled-primary-500"
              type="submit"
              disabled={startingOAuth || completingOAuth || signingInWithPassword}
            >
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

            {#if !isDevContainer}
              <p class="text-sm text-surface-700-300">
                Sign-in completes automatically after browser authentication. No callback URL copy and paste is required.
              </p>
            {:else}
              <p class="text-sm text-surface-700-300">
                Dev container mode: copy the URL below into your host browser, then paste the callback URL back.
              </p>
            {/if}

            {#if isDevContainer && authorizationUrl}
              <div class="space-y-2">
                <label class="label" for="authUrl">Authorization URL</label>
                <input
                  class="input"
                  id="authUrl"
                  type="text"
                  readonly
                  value={authorizationUrl}
                  onclick={(e) => (e.target as HTMLInputElement).select()}
                />
                <p class="text-xs text-surface-700-300">Click to select, then copy and open in your browser.</p>
              </div>
            {/if}

            {#if waitingForCallback}
              {#if isDevContainer}
                <div class="space-y-2">
                  <label class="label" for="callbackUrl">Callback URL</label>
                  <input
                    class="input"
                    id="callbackUrl"
                    type="text"
                    bind:value={manualCallbackUrl}
                    placeholder="singularity://oauth-callback?..."
                  />
                  <button
                    class="btn preset-filled-primary-500"
                    type="button"
                    onclick={handleManualCallback}
                    disabled={completingOAuth || !manualCallbackUrl.trim()}
                  >
                    {#if completingOAuth}
                      Completing Login...
                    {:else}
                      Complete Sign-In
                    {/if}
                  </button>
                </div>
              {:else}
                <p class="text-sm text-surface-700-300">Waiting for browser callback. You can restart sign-in at any time.</p>
              {/if}
            {/if}
          </form>
        {:else}
          <form class="space-y-3" onsubmit={startPasswordLogin}>
            <label class="label" for="username">Username</label>
            <input
              class="input"
              id="username"
              type="text"
              bind:value={username}
              required
            />

            <label class="label" for="password">Password</label>
            <input
              class="input"
              id="password"
              type="password"
              bind:value={password}
              required
            />

            <button
              class="btn preset-filled-primary-500"
              type="submit"
              disabled={signingInWithPassword || startingOAuth || completingOAuth}
            >
              {#if signingInWithPassword}
                Signing In...
              {:else}
                Sign In with Password
              {/if}
            </button>
          </form>
        {/if}
      </section>
    {/if}

    {#if errorMessage}
      <p class="card p-3 text-sm preset-filled-error-500">{errorMessage}</p>
    {/if}

    {#if infoMessage}
      <p class="card p-3 text-sm preset-filled-success-500">{infoMessage}</p>
    {/if}
  </section>
</main>
