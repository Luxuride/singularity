<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";

  interface Props {
    room: MatrixChatSummary;
    depth: number;
    isSelected: boolean;
    hasChildren: boolean;
    isExpanded: boolean;
    onSelect?: (roomId: string) => void;
    onToggleSpace?: (roomId: string) => void;
  }

  let {
    room,
    depth,
    isSelected,
    hasChildren,
    isExpanded,
    onSelect,
    onToggleSpace,
  }: Props = $props();

  const isSpace = $derived(room.kind === "space");

  const indentation = $derived(`${Math.max(0, depth) * 0.9}rem`);

  function handleClick() {
    if (isSpace) {
      if (hasChildren) {
        onToggleSpace?.(room.roomId);
      }
      return;
    }

    onSelect?.(room.roomId);
  }
</script>

<li>
  <button
    type="button"
    class="w-full text-left p-2 rounded transition-colors"
    class:hover:bg-surface-200-800={!isSpace}
    class:opacity-90={isSpace}
    class:cursor-pointer={!isSpace || hasChildren}
    class:cursor-default={isSpace && !hasChildren}
    class:bg-primary-100-900={isSelected}
    style={`padding-left: calc(0.5rem + ${indentation});`}
    onclick={handleClick}
  >
    <div class="flex items-start justify-between gap-2">
      <p class="font-medium truncate">
        {#if isSpace}
          {isExpanded ? "▼" : "▶"}
        {/if}
        {room.displayName}
      </p>
      {#if room.kind === "space"}
        <span class="text-[10px] uppercase tracking-wide text-surface-700-300">Space</span>
      {/if}
    </div>

    {#if room.kind === "space"}
      <p class="text-xs text-surface-700-300">
        {hasChildren ? "Contains rooms" : "No child rooms"}
      </p>
    {:else}
      <p class="text-xs text-surface-700-300">
        {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
      </p>
    {/if}
  </button>
</li>
