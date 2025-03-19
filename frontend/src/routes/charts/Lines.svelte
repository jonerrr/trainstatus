<script lang="ts">
	import { line } from 'd3-shape';
	import { getContext } from 'svelte';
	import { pushState } from '$app/navigation';
	import { type Trip } from '$lib/trips.svelte';
	import { type Route } from '$lib/static';

	const { xGet, yGet, data } = getContext('LayerCake');

	interface Props {
		routes?: Route[];
		stop_points?: boolean;
	}

	const { routes = [], stop_points = $bindable(false) }: Props = $props();

	const path = $derived(line().x($xGet).y($yGet));

	function open_trip(trip: Trip) {
		pushState('', { modal: 'trip', data: trip });
	}
</script>

<!-- Draw a path for each train trip -->
{#each $data as group}
	<!-- {@const tripColor = getTripColor(group.trip)} -->
	{@const tripColor = '#' + routes.find((r) => r.id === group.trip.route_id)!.color}

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
		{#each group.points as point}
			<circle
				cx={$xGet(point)}
				cy={$yGet(point)}
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
