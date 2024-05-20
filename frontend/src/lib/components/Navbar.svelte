<script lang="ts">
	import { createSeparator, melt, type CreateSeparatorProps } from '@melt-ui/svelte';
	import { Home, Clock, CircleAlert } from 'lucide-svelte';
	import NavbarButton from '$lib/components/NavbarButton.svelte';
</script>

<div class="fixed bottom-0 h-12 w-full bg-zinc-800">
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
</div>
