<script lang="ts">
  import type { MatrixDeviceInfo } from "$lib/chats/types";
  import { deviceTrustClass, deviceTrustLabel } from "./helpers";

  interface Props {
    device: MatrixDeviceInfo;
    isRequesting: boolean;
    onVerify?: (device: MatrixDeviceInfo) => void;
  }

  let { device, isRequesting, onVerify }: Props = $props();

  const handleVerify = () => {
    onVerify?.(device);
  };
</script>

<li class="card p-3 preset-outlined-surface-300-700 bg-surface-100-900 space-y-1">
  <div class="flex items-center justify-between gap-2">
    <div class="text-sm font-medium truncate">
      {device.displayName ?? device.deviceId}
      {#if device.displayName}
        <span class="text-xs text-surface-600-400 ml-1 font-mono">{device.deviceId}</span>
      {/if}
    </div>
    <span class="rounded px-2 py-0.5 text-xs shrink-0 {deviceTrustClass(device.trust)}">
      {deviceTrustLabel(device.trust)}
    </span>
  </div>
  {#if device.ed25519Fingerprint}
    <p class="text-xs font-mono text-surface-600-400 break-all">{device.ed25519Fingerprint}</p>
  {/if}
  {#if device.trust === "notVerified"}
    <button
      type="button"
      class="btn preset-tonal text-xs w-full mt-1"
      disabled={isRequesting}
      onclick={handleVerify}
    >
      {isRequesting ? "Requesting..." : "Request Verification"}
    </button>
  {/if}
</li>
