<script lang="ts">
	import { getContext } from 'svelte';
	import { line, curveLinear } from 'd3-shape';

	const { xGet, yGet, data } = getContext('LayerCake');
	export let stroke = '#FFF';
	// Debug the path creation
	// $: console.log('Data in Lines:', $data);
	// $: if ($data && $data.length > 0 && $data[0].points && $data[0].points.length > 0) {
	// 	console.log('Sample point:', $data[0].points[0]);
	// 	console.log('xGet sample result:', $xGet($data[0].points[0]));
	// 	console.log('yGet sample result:', $yGet($data[0].points[0]));
	// 	try {
	// 		const pathString = path($data[0].points);
	// 		console.log('Generated path string:', pathString);
	// 	} catch (e) {
	// 		console.error('Path generation error:', e);
	// 	}
	// }

	// Create a line generator for each trip with defined() to handle missing data
	// $: path = line()
	// 	.x((d) => {
	// 		const x = $xGet(d);
	// 		console.log('Point x value:', d.time, 'transformed:', x);
	// 		return isNaN(x) ? null : x;
	// 	})
	// 	.y((d) => {
	// 		const y = $yGet(d);
	// 		console.log('Point y value:', d.stop_name, 'transformed:', y);
	// 		return isNaN(y) ? null : y;
	// 	})
	// 	.defined((d) => $xGet(d) !== null && !isNaN($xGet(d)) && $yGet(d) !== null && !isNaN($yGet(d)))
	// 	.curve(curveLinear);
	$: path = line().x($xGet).y($yGet).curve(curveLinear);

	// Tooltip data
	let tooltipData = null;
	let tooltipX = 0;
	let tooltipY = 0;

	function showTooltip(event, stop) {
		tooltipData = stop;
		tooltipX = event.clientX;
		tooltipY = event.clientY;
	}

	function hideTooltip() {
		tooltipData = null;
	}

	// // Format time for display
	function formatTime(time) {
		if (!time) return '';
		const date = new Date(time);
		return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}
	// console.log($data[0], lineGenerator($data[0].points));
</script>

<!-- Draw a path for each train trip -->
{#each $data as trip}
	{#if trip.points && trip.points.length > 1}
		<path
			class="path-line"
			d={path(trip.points)}
			fill="none"
			{stroke}
			stroke-width="2"
			opacity="0.8"
			on:mouseover={(e) => showTooltip(e, trip.points[0])}
			on:mouseout={hideTooltip}
		/>

		{#each trip.points as point}
			<circle
				cx={$xGet(point)}
				cy={$yGet(point)}
				r="3"
				fill={stroke}
				stroke="#fff"
				stroke-width="1"
			/>
		{/each}
	{/if}
{/each}
{#if tooltipData}
	<g transform="translate({tooltipX}, {tooltipY})">
		<rect
			x={-80}
			y={-50}
			width="160"
			height={tooltipData.stop_name ? 70 : 40}
			fill="#222"
			stroke="#fff"
			stroke-width="1"
			rx="4"
			ry="4"
		/>
		<text x="0" y="-30" text-anchor="middle" fill="#fff" font-size="12px">
			Trip: {tooltipData.trip_id}
		</text>
		{#if tooltipData.stop_name}
			<text x="0" y="-15" text-anchor="middle" fill="#fff" font-size="12px">
				Stop: {tooltipData.stop_name}
			</text>
			<text x="0" y="0" text-anchor="middle" fill="#fff" font-size="12px">
				Time: {formatTime(tooltipData.time)}
			</text>
		{/if}
	</g>
{/if}
