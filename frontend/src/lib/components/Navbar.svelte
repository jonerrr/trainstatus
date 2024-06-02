<script lang="ts">
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import NavbarButton from '$lib/components/NavbarButton.svelte';
</script>

<nav class="fixed bottom-0 h-16 w-full bg-neutral-800">
	<div class="flex h-full justify-center">
		<NavbarButton>
			<Home class="m-auto h-full" />
		</NavbarButton>
		<NavbarButton href="/alerts">
			<CircleAlert class="m-auto h-full" />
		</NavbarButton>
		<NavbarButton href="/stops">
			<Clock class="m-auto h-full" />
		</NavbarButton>
	</div>
</nav>
