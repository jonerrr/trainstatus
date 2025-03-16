<script lang="ts">
	import { getContext } from 'svelte';
	import dayjs from 'dayjs';
	import { timeMinute } from 'd3-time';

	const { height, xScale } = getContext('LayerCake');

	const formatTime = (time: Date) => dayjs(time).format('h:mm A');

	// not sure if this could be undefined
	$: ticks = timeMinute.every(10)?.range($xScale.domain()[0], $xScale.domain()[1]) ?? [];
</script>

<g class="axis x-axis" transform="translate(0, {$height})">
	<!-- <line x1={0} y1={0} x2={$width} y2={0} stroke="#999" /> -->

	{#each ticks as tick}
		<g transform="translate({$xScale(tick)}, 0)">
			<!-- <line y1={0} y2={6} stroke="#e5e5e5" /> -->
			<text y={16} text-anchor="middle" fill="#e5e5e5" font-size="12px">
				{formatTime(tick)}
			</text>
		</g>
		<line
			x1={$xScale(tick)}
			y1={0}
			x2={$xScale(tick)}
			y2={-$height}
			stroke="#e5e5e5"
			class="gridline"
		/>
	{/each}
</g>

<style>
	.gridline {
		opacity: 0.5;
	}
</style>
