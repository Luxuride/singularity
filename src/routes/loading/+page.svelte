<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  import { matrixLogout, matrixSessionStatus } from "$lib/auth/api";
  import { matrixTriggerRoomUpdate } from "$lib/chats/api";
  import type { MatrixRoomRefreshCompleteEvent } from "$lib/chats/types";

  let errorMessage = $state("");
  let loadingMessage = $state("Checking session...");

  const AUTH_LOGOUT_COMPLETE_EVENT = "matrix://auth/logout/complete";

  const ROOM_REFRESH_COMPLETE_EVENT = "matrix://rooms/refresh/complete";
  const ROOM_REFRESH_WAIT_TIMEOUT_MS = 10000;

  async function waitForRoomRefreshComplete(): Promise<boolean> {
    const refreshCompleted = await Promise.race([
      new Promise<boolean>((resolve) => {
        void (async () => {
          let unlisten: (() => void) | null = null;
          unlisten = await listen<MatrixRoomRefreshCompleteEvent>(
            ROOM_REFRESH_COMPLETE_EVENT,
            () => {
              unlisten?.();
              resolve(true);
            },
          );
        })();
      }),
      new Promise<boolean>((resolve) => {
        setTimeout(() => resolve(false), ROOM_REFRESH_WAIT_TIMEOUT_MS);
      }),
    ]);

    return refreshCompleted;
  }

  onMount(() => {
    void (async () => {
      try {
        const mode = page.url.searchParams.get("mode");
        if (mode === "logout") {
          loadingMessage = "Signing out...";
          const logoutComplete = new Promise<void>((resolve) => {
            void (async () => {
              let unlisten: (() => void) | null = null;
              unlisten = await listen<boolean>(AUTH_LOGOUT_COMPLETE_EVENT, () => {
                unlisten?.();
                resolve();
              });
            })();
          });
          await matrixLogout();
          await logoutComplete;
          await goto("/");
          return;
        }

        const status = await matrixSessionStatus();
        if (!status.authenticated) {
          loadingMessage = "Preparing sign-in...";
          await goto("/");
          return;
        }

        loadingMessage = "Loading rooms...";

        try {
          await matrixTriggerRoomUpdate();
        } catch {
          // If refresh fails, still attempt to load cached rooms.
        }

        const refreshCompleted = await waitForRoomRefreshComplete();
        if (!refreshCompleted) {
          loadingMessage = "Loading cached rooms...";
        }

        await goto("/chats");
      } catch (error) {
        errorMessage = error instanceof Error ? error.message : "Failed to load session";
      }
    })();
  });
</script>

<main class="min-h-screen grid place-items-center p-4">
  <section class="card w-full max-w-md p-4 preset-outlined-surface-200-800 bg-surface-50-950">
    {#if errorMessage}
      <p class="text-sm preset-filled-error-500">{errorMessage}</p>
    {:else}
      <p class="text-sm text-surface-700-300">{loadingMessage}</p>
    {/if}
  </section>
</main>
