<script lang="ts">
	import { type ChartPoint, chart_context } from '#lib/charts/context.js';
	import type { Route, Trip } from '#lib/client/index.js';
	import { open_modal } from '#lib/url_params.svelte.js';

	import { line } from 'd3-shape';

	const cake = chart_context();

	interface Props {
		routes?: Route[];
		stop_points?: boolean;
	}

	const { routes = [], stop_points = $bindable(false) }: Props = $props();

	const path = $derived(
		line<ChartPoint>()
			.x((point) => cake.xGet(point))
			.y((point) => cake.yGet(point))
	);

	function open_trip(trip: Trip) {
		open_modal({ ...trip, type: 'trip' });
	}
</script>

<!-- Draw a path for each train trip -->
{#each cake.data as group (group.trip.id)}
	{@const tripColor = routes.find((r) => r.id === group.trip.route_id)!.color}

	<!-- Invisible wider path for easier clicking -->
	<path
		class="path-hitarea"
		d={path(group.points)}
		fill="none"
		stroke="transparent"
		stroke-width="15"
		opacity="0"
		role="button"
		tabindex="0"
		aria-label="View details for trip {group.trip.id}"
		onclick={() => open_trip(group.trip)}
		onkeydown={(e) => e.key === 'Enter' && open_trip(group.trip)}
	/>

	<!-- Visible path for display only -->
	<path
		class="path-line"
		d={path(group.points)}
		fill="none"
		stroke={tripColor}
		stroke-width="2"
		opacity="1"
		pointer-events="none"
	/>
	{#if stop_points}
		{#each group.points as point (`${point.stop_id}-${point.time.getTime()}`)}
			<circle
				cx={cake.xGet(point)}
				cy={cake.yGet(point)}
				r="3"
				fill={tripColor}
				stroke="#fff"
				stroke-width="1"
			/>
		{/each}
	{/if}
{/each}

<style>
	.path-line:hover {
		stroke-width: 4;
		opacity: 1;
	}

	/* Style for the hit area - cursor pointer but invisible */
	.path-hitarea {
		cursor: pointer;
	}

	/* This ensures the visible path still gets highlighted when hovering on the hit area */
	.path-hitarea:hover + .path-line {
		stroke-width: 4;
		opacity: 1;
	}
</style>
