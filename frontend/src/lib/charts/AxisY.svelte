<script lang="ts">
	import { chart_context } from '#lib/charts/context.js';

	const cake = chart_context();

	const ticks = $derived(cake.yScale.domain());
</script>

<g class="axis y-axis">
	{#each ticks as tick (tick)}
		<g transform="translate(0, {cake.yScale(tick)})">
			<line x1={-3} x2={2} stroke="#e5e5e5" />
			<text x={-5} y={4} fill="#e5e5e5" text-anchor="end" font-size="11px" class="stop-name">
				{tick.length > 25 ? tick.substring(0, 22) + '...' : tick}
			</text>
		</g>
		<line
			x1={0}
			y1={cake.yScale(tick)}
			x2={cake.width}
			y2={cake.yScale(tick)}
			stroke="#e5e5e5"
			class="gridline"
		/>
	{/each}
</g>

<style>
	.stop-name {
		max-width: 110px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.gridline {
		opacity: 0.5;
	}
</style>
