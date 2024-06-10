<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import Trigger from '$lib/components/RouteAlert/Trigger.svelte';

	export let route_ids: string[] = [];
	export let title: string = 'Alerts';
	// export let accordion: boolean = false;

	// for if no route ids are supplied
	const all_routes_ids = [
		'1',
		'2',
		'3',
		'4',
		'5',
		'6',
		'7',
		'A',
		'C',
		'E',
		'B',
		'D',
		'F',
		'M',
		'G',
		'J',
		'Z',
		'L',
		'N',
		'Q',
		'R',
		'W',
		'H',
		'FS',
		'GS',
		'SI'
	];

	if (!route_ids.length) route_ids = all_routes_ids;
</script>

<!-- TODO: allow user to customize list length or add buttom to add /subtract shown pinned routes -->
<div
	class={`overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 max-h-[calc(100dvh-8rem)]`}
>
	<div class="flex text-lg justify-between self-center w-full">
		<div class="font-semibold text-indigo-300">{title}</div>
	</div>
	<div class="flex flex-col mt-1 gap-1">
		{#each route_ids as route_id (route_id)}
			<div
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl hover:bg-neutral-900 px-1"
				transition:slide={{ easing: quintOut, axis: 'y' }}
			>
				<Trigger {route_id} />
			</div>
		{/each}
	</div>
</div>
