<script lang="ts">
	import { chartContext, type ChartPoint, type ChartSeries } from '#lib/charts/context.js';
	import type { CatalogRoute } from '#lib/client/index.js';
	import { openModal } from '#lib/url-params.svelte.js';

	import { line } from 'd3-shape';

	const cake = chartContext();

	interface Props {
		routes?: CatalogRoute[];
		stopPoints?: boolean;
	}

	const { routes = [], stopPoints = $bindable(false) }: Props = $props();

	const path = $derived(
		line<ChartPoint>()
			.x((point) => cake.xGet(point))
			.y((point) => cake.yGet(point))
	);

	function openTrip(group: ChartSeries) {
		openModal({ type: 'trip', source: group.source, routeId: group.routeId, id: group.trip.id });
	}
</script>

<!-- Trips can extend past the x domain; keep them out of the axis label margins. -->
<clipPath id="chart-plot-area">
	<rect y={-4} width={cake.width} height={cake.height + 8} />
</clipPath>

<g clip-path="url(#chart-plot-area)">
	<!-- Draw a path for each train trip -->
	{#each cake.data as group (`${group.source}:${group.trip.id}`)}
		{const tripColor = $derived(
			routes.find((r) => r.id === group.routeId && r.data.source === group.source)?.color
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
			onclick={() => openTrip(group)}
			onkeydown={(e) => e.key === 'Enter' && openTrip(group)}
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
		{#if stopPoints}
			{#each group.points as point (point.stopKey)}
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
</g>

<style>
	.path-hitarea {
		cursor: pointer;
	}

	@media (hover: hover) and (pointer: fine) {
		.path-hitarea:hover + .path-line {
			stroke-width: 3;
		}
	}
</style>
