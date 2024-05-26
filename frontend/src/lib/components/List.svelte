<script lang="ts">
	import { createScrollArea, melt } from '@melt-ui/svelte';
	import { LoaderCircle } from 'lucide-svelte';
	import { writable } from 'svelte/store';

	// export let items = writable([]);
	export let loading = true;
	// export let maxH = '';

	// export let title;

	const {
		elements: { root, content, viewport, corner, scrollbarY, thumbY }
	} = createScrollArea({
		// TODO: test auto
		type: 'auto',
		dir: 'ltr'
	});
</script>

<div
	use:melt={$root}
	class={`relative w-full ${$$restProps.class ?? ''} overflow-hidden rounded-md text-white shadow-lg`}
>
	<div use:melt={$viewport} class="h-full w-full rounded-[inherit]">
		<div use:melt={$content}>
			<div>
				<slot name="header" />
				{#if loading}
					<div class="flex w-full justify-center">
						<LoaderCircle class="animate-spin w-8 h-8 text-indigo-300" />
					</div>
				{:else}
					<slot />
				{/if}
			</div>
		</div>
	</div>
	<div
		use:melt={$scrollbarY}
		class="flex h-full w-2.5 touch-none select-none border-l border-l-transparent bg-neutral-300/10 p-px transition-colors"
	>
		<div use:melt={$thumbY} class="relative flex-1 rounded-full bg-neutral-300/50" />
	</div>
	<div use:melt={$corner} />
</div>
