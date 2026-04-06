<script lang="ts">
  interface Props {
    userId: string;
  }

  let { userId }: Props = $props();
</script>

<section class="card p-4 md:p-6 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950">
  <header>
    <div>
      <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Singularity</p>
      <h1 class="h3">Chats</h1>
      <p class="text-xs text-surface-700-300">{userId}</p>
    </div>
  </header>
</section>
