<script lang="ts">
        import { page } from "$app/state";

        const { children } = $props();

        const settingsNavItems = [
            { href: "/settings", label: "General" },
            { href: "/settings/security", label: "Security" },
        ] as const;

        function isActivePath(href: string): boolean {
            return page.url.pathname === href;
        }
</script>

<section class="space-y-4">
    <nav class="card p-2 preset-outlined-surface-200-800 bg-surface-50-950">
        <ul class="flex flex-wrap gap-2">
            {#each settingsNavItems as item (item.href)}
                <li>
                    <a
                        class="btn text-sm"
                        class:preset-filled-primary-500={isActivePath(item.href)}
                        class:preset-tonal={!isActivePath(item.href)}
                        href={item.href}
                    >
                        {item.label}
                    </a>
                </li>
            {/each}
        </ul>
    </nav>

    {@render children()}
</section>