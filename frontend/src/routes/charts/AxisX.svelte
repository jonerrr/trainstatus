<script lang="ts">
	import { getContext } from 'svelte';
	import dayjs from 'dayjs';
	import { timeMinute } from 'd3-time';

	const { width, height, xScale } = getContext('LayerCake');

	const formatTime = (time: Date) => dayjs(time).format('h:mm A');

	// Calculate tick values - every 10 minutes
	$: ticks = timeMinute.every(10).range($xScale.domain()[0], $xScale.domain()[1]);
</script>

<g class="axis x-axis" transform="translate(0, {$height})">
	<line x1={0} y1={0} x2={$width} y2={0} stroke="#999" />

	<!-- Vertical gridlines -->
	{#each ticks as tick}
		<line
			x1={$xScale(tick)}
			y1={0}
			x2={$xScale(tick)}
			y2={-$height}
			stroke="#e5e5e5"
			class="gridline"
		/>
	{/each}

	{#each ticks as tick}
		<g transform="translate({$xScale(tick)}, 0)">
			<line y1={0} y2={6} stroke="#999" />
			<text y={16} text-anchor="middle" fill="#999" font-size="12px">
				{formatTime(tick)}
			</text>
		</g>
	{/each}

	<!-- <text x={$width / 2} y={30} fill="#999" text-anchor="middle" font-size="14px" font-weight="bold">
		Time
	</text> -->
</g>

<style>
	.gridline {
		opacity: 0.5;
	}
</style>
