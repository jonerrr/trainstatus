<script lang="ts">
	import { chart_context } from '#lib/charts/context.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	import { timeMinute } from 'd3-time';
	import dayjs from 'dayjs';

	const current_time = getCurrentTime();

	const cake = chart_context();

	const formatDate = (time: Date) => dayjs(time).format('YYYY-MM-DD');
	const formatTime = (time: Date) => dayjs(time).format('h:mm A');

	const { interval = 15, current_time_line = true } = $props();

	// not sure if this could be undefined
	const ticks = $derived(
		timeMinute.every(interval)?.range(cake.xScale.domain()[0], cake.xScale.domain()[1]) ?? []
	);
</script>

{#if current_time_line}
	<g transform="translate({cake.xScale(new Date())}, 0)">
		<text y={-6} text-anchor="middle" fill="#e5e5e5" font-size="12px">{formatTime(new Date())}</text
		>
		<line y1={0} y2={cake.height} stroke="#e5e5e5" />
	</g>
{/if}

<g transform="translate(0, {cake.height})">
	<text x={-65} y={20} text-anchor="middle" fill="#e5e5e5" font-size="12px">
		{formatDate(new Date(current_time.ms))} -
	</text>
</g>

<g class="axis x-axis" transform="translate(0, {cake.height})">
	{#each ticks as tick (tick.getTime())}
		{const x = $derived(cake.xScale(tick))}
		<g transform="translate({x}, 0)">
			<text y={20} text-anchor="middle" fill="#e5e5e5" font-size="12px">
				{formatTime(tick)}
			</text>
		</g>
		<line x1={x} y1={0} x2={x} y2={-cake.height} stroke="#e5e5e5" class="gridline" />
	{/each}
</g>

<style>
	.gridline {
		opacity: 0.5;
	}
</style>
