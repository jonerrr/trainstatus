<script lang="ts">
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import NavbarButton from '$lib/components/NavbarButton.svelte';
</script>

<nav class="fixed bottom-0 h-16 w-full bg-neutral-800">
	<div class="grid grid-cols-3 h-full">
		<NavbarButton>
			<Home />
		</NavbarButton>
		<NavbarButton href="/alerts">
			<CircleAlert />
		</NavbarButton>
		<NavbarButton href="/stops">
			<Clock />
		</NavbarButton>
	</div>
</nav>
