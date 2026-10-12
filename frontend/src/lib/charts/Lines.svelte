<script lang="ts">
	import { chart_context, type ChartPoint, type ChartSeries } from '#lib/charts/context.js';
	import type { CatalogRoute } from '#lib/client/index.js';
	import { open_modal } from '#lib/url_params.svelte.js';

	import { line } from 'd3-shape';

	const cake = chart_context();

	interface Props {
		routes?: CatalogRoute[];
		stop_points?: boolean;
	}

	const { routes = [], stop_points = $bindable(false) }: Props = $props();

	const path = $derived(
		line<ChartPoint>()
			.x((point) => cake.xGet(point))
			.y((point) => cake.yGet(point))
	);

	function open_trip(group: ChartSeries) {
		open_modal({ type: 'trip', source: group.source, route_id: group.route_id, id: group.trip.id });
	}
</script>

<!-- Draw a path for each train trip -->
{#each cake.data as group (`${group.source}:${group.trip.id}`)}
	{const tripColor = $derived(
		routes.find((r) => r.id === group.route_id && r.data.source === group.source)?.color
	)}

	{const tripPath = $derived(path(group.points))}

	<!-- Invisible wider path for easier clicking -->
	<path
		class="path-hitarea"
		d={tripPath}
		fill="none"
		stroke="transparent"
		stroke-width="15"
		opacity="0"
		role="button"
		tabindex="0"
		aria-label="View details for trip {group.trip.id}"
		onclick={() => open_trip(group)}
		onkeydown={(e) => e.key === 'Enter' && open_trip(group)}
	/>

	<!-- Visible path for display only -->
	<path
		class="path-line"
		d={tripPath}
		fill="none"
		stroke={tripColor}
		stroke-width="2"
		opacity="1"
		pointer-events="none"
	/>
	{#if stop_points}
		{#each group.points as point (point.stop_key)}
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
