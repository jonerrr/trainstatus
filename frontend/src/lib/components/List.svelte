<script lang="ts">
	import { persisted } from 'svelte-persisted-store';
	import { onDestroy, onMount, tick } from 'svelte';
	import { BusFront, TrainFront } from 'lucide-svelte';
	import { createTabs, melt } from '@melt-ui/svelte';

	// manage the min/max height of the list
	export let manage_height: boolean = true;
	export let title: string = 'List';
	export let tab_value = persisted(`${title.toLowerCase()}_tab`, 'Train');

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'Train',
		value: tab_value
	});

	let list_el: HTMLDivElement;
	export function scrollIntoView() {
		list_el.scrollIntoView({ behavior: 'smooth' });
	}

	let list_height = 0;
	let title_height: number = 0;
	let interval: number;

	if (manage_height) {
		onMount(() => {
			setInterval(async () => {
				if (list_el == null) return;

				const els = Array.from(list_el.querySelectorAll('#list-item')).slice(0, 3);
				// await tick();
				list_height = els.reduce((h, e) => e.offsetHeight + h, 0);
				// list_height += 10;
			}, 5);
		});

		onDestroy(() => {
			clearInterval(interval);
		});
	}

	// ${show_search ? 'max-h-[calc(100dvh-11rem)] overflow-auto' : 'max-h-[calc(100dvh-4rem)]'}

	const list_classes = `flex flex-col overflow-auto ${$$props.class} ?? 'max-h-[calc(100dvh-4rem)]'}`;
</script>

<div
	use:melt={$root}
	bind:this={list_el}
	style={manage_height ? `min-height: ${list_height}px; max-height: ${list_height}px;` : ''}
	class={`relative flex flex-col text-indigo-200 overflow-auto overscroll-nonebg-neutral-900 ${$$props.class} ?? ''}`}
>
	<div
		id="list-item"
		bind:clientHeight={title_height}
		class="flex z-40 w-full md:w-[60%] fixed justify-between bg-neutral-800 p-1 items-center"
	>
		<div class="font-bold text-lg text-indigo-300 flex gap-1 items-center">
			{title}
			<!-- for geolocate -->
			<slot name="title" />
		</div>

		<div
			use:melt={$list}
			class="grid grid-cols-2 bg-neutral-900 rounded shrink-0 overflow-x-auto text-indigo-100 border border-neutral-500"
			aria-label="List"
		>
			<button
				use:melt={$trigger('Train')}
				class="trigger p-1 px-2 rounded-l relative border-neutral-400 border-r data-[state=active]:bg-indigo-800"
			>
				<TrainFront />
			</button>
			<button
				use:melt={$trigger('Bus')}
				class="trigger p-1 px-2 rounded-r relative border-neutral-400 border-l data-[state=active]:bg-indigo-800"
			>
				<BusFront />
			</button>
		</div>
	</div>

	{#if $value === 'Train'}
		<div
			style={`padding-top: ${title_height}px;`}
			class={list_classes}
			use:melt={$content('Train')}
		>
			<slot name="train" />
		</div>
	{:else if $value === 'Bus'}
		<div style={`padding-top: ${title_height}px;`} class={list_classes} use:melt={$content('Bus')}>
			<slot name="bus" />
		</div>
	{/if}

	<slot />

	<!-- TODO: Figure out why list height is wrong when using these -->
	<!-- <div use:melt={$content('Train')}>
		<slot />
	</div>
	<div use:melt={$content('Bus')}>
		<slot name="bus" />
	</div> -->
</div>
