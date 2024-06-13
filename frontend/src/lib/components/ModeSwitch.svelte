<script lang="ts">
	import { createTabs, melt } from '@melt-ui/svelte';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'Train'
	});

	const triggers = ['Train', 'Bus'];

	const [send, receive] = crossfade({
		duration: 250,
		easing: cubicInOut
	});
</script>

<div
	use:melt={$root}
	class="flex border border-neutral-800 flex-col rounded-xl shadow-lg data-[orientation=vertical]:flex-row bg-neutral-900/50 text-indigo-400"
>
	<div
		use:melt={$list}
		class="flex shrink-0 overflow-x-auto text-indigo-100
data-[orientation=vertical]:flex-col data-[orientation=vertical]:border-r"
		aria-label="List"
	>
		{#each triggers as triggerItem}
			<button use:melt={$trigger(triggerItem)} class="trigger relative">
				{triggerItem}
				{#if $value === triggerItem}
					<div
						in:send={{ key: 'trigger' }}
						out:receive={{ key: 'trigger' }}
						class="absolute bottom-1 left-1/2 h-1 w-full -translate-x-1/2 rounded-full bg-indigo-400"
					/>
				{/if}
			</button>
		{/each}
	</div>
	<div use:melt={$content('Trains')} class="bg-neutral-900/50 p-2">
		<slot name="trains" />
	</div>
	<div use:melt={$content('Buses')} class=" bg-neutral-900/50 p-2">
		<slot name="buses" />
	</div>
</div>
