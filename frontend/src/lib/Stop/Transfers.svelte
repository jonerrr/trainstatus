<script lang="ts">
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import BusArrow from '$lib/Stop/BusArrow.svelte';
	import { open_modal } from '$lib/url_params.svelte';
	import { main_route_stops } from '$lib/util.svelte';

	import type { CompassDirection, Source, Transfer } from '@trainstatus/client';

	interface Props {
		stop_source: Source;
		transfers: Transfer[];
	}

	const { transfers, stop_source }: Props = $props();

	const DIRECTION_ORDER: Record<CompassDirection, number> = {
		n: 0,
		n_e: 1,
		e: 2,
		s_e: 3,
		s: 4,
		s_w: 5,
		w: 6,
		n_w: 7,
		unknown: 8
	};
	const sorted_transfers = $derived(
		transfers
			.map((t) => ({
				...t,
				stop: page.data.stops_by_id[t.to_stop_source][t.to_stop_id]
			}))
			.sort((a, b) => {
				// Prioritize transfers from the same source as the current stop
				if (a.stop.data.source === stop_source && b.stop.data.source !== stop_source) return -1;
				if (b.stop.data.source === stop_source && a.stop.data.source !== stop_source) return 1;

				// Then sort by direction
				const dir_a =
					a.stop.data.source === 'mta_bus'
						? DIRECTION_ORDER[a.stop.data.direction]
						: DIRECTION_ORDER.unknown;
				const dir_b =
					b.stop.data.source === 'mta_bus'
						? DIRECTION_ORDER[b.stop.data.direction]
						: DIRECTION_ORDER.unknown;
				if (dir_a !== dir_b) return dir_a - dir_b;

				// Finally sort by route ID
				return a.stop.routes[0]?.route_id.localeCompare(b.stop.routes[0]?.route_id ?? '') ?? 0;
			})
	);
</script>

<div class="flex flex-col bg-neutral-900 px-1 pb-1">
	<div class="font-medium">Transfers:</div>
	<div class="flex items-center gap-2 overflow-x-auto">
		{#each sorted_transfers as { stop, to_stop_source } (stop.id)}
			<button
				class="flex items-center gap-1 rounded-sm bg-neutral-800 p-1 shadow-2xl transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
				onclick={() => open_modal({ type: 'stop', ...$state.snapshot(stop) })}
			>
				{#if stop.data.source === 'mta_bus'}
					<BusArrow direction={stop.data.direction} size="1rem" />
				{/if}
				{#each main_route_stops(stop.routes) as route_stop}
					<Icon
						width={24}
						height={24}
						link={false}
						route={page.data.routes_by_id[to_stop_source][route_stop.route_id]}
					/>
				{/each}
			</button>
		{/each}
	</div>
</div>
