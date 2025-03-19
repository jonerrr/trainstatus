<script lang="ts">
	import { getContext } from 'svelte';
	import dayjs from 'dayjs';
	import { timeMinute } from 'd3-time';
	import { current_time } from '$lib/util.svelte';

	const { height, xScale } = getContext('LayerCake');

	const formatDate = (time: Date) => dayjs(time).format('YYYY-MM-DD');
	const formatTime = (time: Date) => dayjs(time).format('h:mm A');

	const { interval = 15, current_time_line = true } = $props();

	// not sure if this could be undefined
	const ticks = $derived(
		timeMinute.every(interval)?.range($xScale.domain()[0], $xScale.domain()[1]) ?? []
	);
</script>

{#if current_time_line}
	<g transform="translate({$xScale(new Date())}, 0)">
		<text y={-6} text-anchor="middle" fill="#e5e5e5" font-size="12px">{formatTime(new Date())}</text
		>
		<line y1={0} y2={$height} stroke="#e5e5e5" />
	</g>
{/if}

<g transform="translate(0, {$height})">
	<text x={-65} y={20} text-anchor="middle" fill="#e5e5e5" font-size="12px">
		{formatDate(new Date(current_time.ms))} -
	</text>
</g>

<g class="axis x-axis" transform="translate(0, {$height})">
	{#each ticks as tick}
		<g transform="translate({$xScale(tick)}, 0)">
			<text y={20} text-anchor="middle" fill="#e5e5e5" font-size="12px">
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
