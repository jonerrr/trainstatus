<script lang="ts">
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import NavbarButton from './NavbarButton.svelte';
</script>

<!-- {#snippet nav_button({ label, href, icon })}
	<a
		aria-label={label}
		{href}
		class={`block w-full h-full m-auto pb-2 ${$page.url.pathname === href ? 'text-fuchsia-700' : 'text-indigo-600'}`}
	>
		<icon class="m-auto h-full" />
	</a>
{/snippet} -->

<nav class="fixed bottom-0 h-16 w-full bg-neutral-800 z-40">
	<div class="flex h-full justify-center">
		<NavbarButton>
			<Home class="m-auto h-full" />
		</NavbarButton>
		<!-- {@render nav_button({ label: 'Home', href: '/', icon: Home })} -->

		<NavbarButton label="Alerts" href="/alerts">
			<CircleAlert class="m-auto h-full" />
		</NavbarButton>
		<NavbarButton label="Stops" href="/stops">
			<Clock class="m-auto h-full" />
		</NavbarButton>
	</div>
</nav>
