<script lang="ts">
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import Skeleton from '$lib/Skeleton.svelte';
	import type { Trip } from '$lib/client';
	import { position_context } from '$lib/resources/positions.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { current_time } from '$lib/url_params.svelte';

	import { ArrowBigRight } from '@lucide/svelte';

	interface Props {
		data: Trip;
	}
	let { data }: Props = $props();

	const source_stop_times = $derived(stop_time_context.getSource(data.data.source));

	const all_trip_stop_times = $derived(source_stop_times?.current.by_trip_id.get(data.id) ?? []);

	const is_loading = $derived(
		!source_stop_times || (source_stop_times.status !== 'ready' && all_trip_stop_times.length === 0)
	);

	const stop_times = $derived(
		all_trip_stop_times.filter((st) => st.arrival.getTime() > current_time.ms)
	);

	const position = $derived(
		position_context.getSource(data.data.source)?.current?.get(data.vehicle_id)
	);

	const route = $derived(page.data.routes_by_id?.[data.data.source]?.[data.route_id]);

	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		switch (data.data.source) {
			case 'mta_bus':
				const stop = page.data.stops_by_id?.[data.data.source]?.[stop_times[0].stop_id];
				const routeStop = stop?.routes.find((r) => r.route_id === data.route_id);
				return routeStop?.data.source === 'mta_bus' ? routeStop.data.headsign : 'Unknown';
			case 'mta_subway':
				const last_st = stop_times[stop_times.length - 1];
				return page.data.stops_by_id?.[data.data.source]?.[last_st.stop_id]?.name ?? 'Unknown';
			default:
				return 'Unknown';
		}
	});

	const current_stop = $derived.by(() => {
		const target_stop_id = position?.stop_id || stop_times[0]?.stop_id;
		if (!stop_times.length) return 'Unknown';
		return page.data.stops_by_id?.[data.data.source]?.[target_stop_id]?.name ?? 'Unknown';
	});
</script>

{#if is_loading}
	<Skeleton lines={2} class="w-full" />
{:else}
	<div class="flex flex-col items-center gap-1 text-left">
		<div class="flex max-w-[95%] items-center gap-1 self-start">
			{#if route}
				<Icon width={36} height={36} {route} link={false} />
			{/if}

			<ArrowBigRight />

			{last_stop}
		</div>

		<div class="flex gap-1 self-start">
			{#if position?.data && 'status' in position.data}
				<span>{(position.data as any).status}</span>
			{/if}
			{current_stop}
		</div>
	</div>
{/if}
