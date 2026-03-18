<script lang="ts">
  import type { MatrixDeviceInfo } from "$lib/chats/types";
  import DeviceItem from "./DeviceItem.svelte";

  interface Props {
    devices: MatrixDeviceInfo[];
    loading: boolean;
    requestingDeviceId: string | null;
    onVerify?: (device: MatrixDeviceInfo) => void;
  }

  let { devices, loading, requestingDeviceId, onVerify }: Props = $props();
</script>

{#if loading}
  <p class="text-sm text-surface-700-300">Loading devices...</p>
{:else if devices.length === 0}
  <p class="text-sm text-surface-700-300">No devices found for this user.</p>
{:else}
  <ul class="space-y-2">
    {#each devices as device (device.deviceId)}
      <DeviceItem
        {device}
        isRequesting={requestingDeviceId === device.deviceId}
        onVerify={onVerify}
      />
    {/each}
  </ul>
{/if}
