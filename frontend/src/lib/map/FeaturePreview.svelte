<script lang="ts">
	import type { MapTarget, ScreenPoint } from './interactions';
	import MapFeatureSummary from './MapFeatureSummary.svelte';

	let {
		target,
		point,
		viewportWidth,
		viewportHeight
	}: { target: MapTarget; point: ScreenPoint; viewportWidth: number; viewportHeight: number } =
		$props();
	let width = $state(280);
	let height = $state(0);
	const left = $derived(
		Math.max(
			8,
			Math.min(
				point.x + 16 + width > viewportWidth ? point.x - width - 16 : point.x + 16,
				viewportWidth - width - 8
			)
		)
	);
	const top = $derived(
		Math.max(
			8,
			Math.min(
				point.y + 16 + height > viewportHeight ? point.y - height - 16 : point.y + 16,
				viewportHeight - height - 8
			)
		)
	);
</script>

<div
	bind:clientWidth={width}
	bind:clientHeight={height}
	data-map-tooltip-kind={target.kind}
	class="pointer-events-none absolute z-20 w-70 max-w-[calc(100%-1rem)] rounded-xl border border-neutral-700 bg-neutral-950/95 p-3 shadow-lg"
	style:left={`${left}px`}
	style:top={`${top}px`}
>
	<MapFeatureSummary {target} />
</div>
