<script lang="ts">
	import { getContext } from 'svelte';

	const { yScale, width } = getContext('LayerCake');

	const ticks = $derived($yScale.domain());
</script>

<g class="axis y-axis">
	{#each ticks as tick, i}
		<g transform="translate(0, {$yScale(tick)})">
			<line x1={-3} x2={2} stroke="#e5e5e5" />
			<text x={-5} y={4} fill="#e5e5e5" text-anchor="end" font-size="11px" class="stop-name">
				{tick.length > 25 ? tick.substring(0, 22) + '...' : tick}
			</text>
		</g>
		<line
			x1={0}
			y1={$yScale(tick)}
			x2={$width}
			y2={$yScale(tick)}
			stroke="#e5e5e5"
			class="gridline"
		/>
	{/each}

	<!-- {#each ticks as tick}
		<line
			x1={0}
			y1={$yScale(tick)}
			x2={$width}
			y2={$yScale(tick)}
			stroke="#e5e5e5"
			class="gridline"
		/>
	{/each} -->

	<!-- <text
		x={-$yScale.range()[0] / 2}
		y={-80}
		fill="#999"
		text-anchor="middle"
		font-size="14px"
		font-weight="bold"
	>
		Stations
	</text> -->
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
