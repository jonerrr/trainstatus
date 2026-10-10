<script lang="ts">
	import { page } from '$app/state';

	import type { Trip } from '#lib/client/index.js';
	import { source_info } from '#lib/resources/index.svelte.js';
	import { awaitingRows } from '#lib/resources/pending.js';
	import { position_context } from '#lib/resources/positions.svelte.js';
	import { stop_time_context } from '#lib/resources/stop_times.svelte.js';
	import Icon from '#lib/Route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getCurrentTime } from '#lib/url_params.svelte.js';
	import { trip_headsign } from '#lib/util.svelte.js';

	import { ArrowBigRight } from '@lucide/svelte';

	const current_time = getCurrentTime();

	interface Props {
		data: Trip;
		acquire_routes?: boolean;
	}
	let { data, acquire_routes = true }: Props = $props();

	const source_stop_times = $derived(stop_time_context.getSource(data.data.source));

	$effect(() => {
		if (!acquire_routes || !source_info[data.data.source].monitor_routes) return;
		return source_stop_times?.hold([data.route_id]);
	});

	const all_trip_stop_times = $derived(source_stop_times?.current.by_trip_id.get(data.id) ?? []);

	const is_loading = $derived(
		awaitingRows(
			source_stop_times,
			all_trip_stop_times.length,
			source_info[data.data.source].monitor_routes ? data.route_id : undefined
		)
	);

	const stop_times = $derived(
		all_trip_stop_times.filter((st) => st.arrival.getTime() > current_time.ms)
	);

	const position = $derived(
		position_context.getSource(data.data.source)?.current?.get(data.vehicle_id)
	);

	const route = $derived(page.data.routes_by_id?.[data.data.source]?.[data.route_id]);

	const last_stop = $derived(
		trip_headsign(data, route, all_trip_stop_times, page.data.stops_by_id?.[data.data.source])
	);

	const current_stop = $derived.by(() => {
		const target_stop_id = position?.stop_id || stop_times[0]?.stop_id;
		if (!stop_times.length) return 'Unknown';
		return page.data.stops_by_id?.[data.data.source]?.[target_stop_id]?.name ?? 'Unknown';
	});
</script>

{#if source_stop_times?.error && !all_trip_stop_times.length}<span class="text-xs text-amber-200"
		>Unavailable</span
	>{:else if is_loading}
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
				<span>{position.data.status}</span>
			{/if}
			{current_stop}
		</div>
	</div>
{/if}
