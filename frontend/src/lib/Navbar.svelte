<script lang="ts">
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import { page } from '$app/stores';

	interface Routes {
		[key: string]: [Icon: typeof Home | typeof Clock | typeof CircleAlert, href: string];
	}

	const routes: Routes = {
		Home: [Home, '/'],
		Alerts: [CircleAlert, '/alerts'],
		Stops: [Clock, '/stops']
	} as const;
</script>

{#snippet nav_button(label: string)}
	{@const [Icon, href] = routes[label]}
	<a
		aria-label={label}
		{href}
		class="grid h-full w-full grid-cols-1 text-center rounded-none text-sm"
		class:text-neutral-200={href === $page.url.pathname}
		class:text-neutral-400={href !== $page.url.pathname}
		class:underline={href === $page.url.pathname}
	>
		<Icon class="m-auto h-full" />
		{label}
	</a>
{/snippet}

<nav class="fixed bottom-0 h-16 w-full bg-neutral-900 z-40">
	<div class="flex h-full justify-center">
		{@render nav_button('Home')}
		{@render nav_button('Alerts')}
		{@render nav_button('Stops')}
	</div>
</nav>
