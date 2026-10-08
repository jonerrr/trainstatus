<script lang="ts">
	import { onMount } from 'svelte';

	import type { Layer } from '@deck.gl/core';
	import { MapLibreOverlay } from '@deck.gl/maplibre';
	import { getMapContext } from 'svelte-maplibre-gl';

	import type { VehiclePicker } from './interactions';
	import type { ActiveVehicle } from './trajectoryArrow';

	let { layers, onpicker }: { layers: Layer[]; onpicker: (picker: VehiclePicker | null) => void } =
		$props();
	const context = getMapContext();
	let overlay = $state.raw<MapLibreOverlay>();
	onMount(() => {
		const map = context.map;
		if (!map) return;
		const control = new MapLibreOverlay({ interleaved: true, layers });
		map.addControl(control);
		overlay = control;
		onpicker({
			pick(point, radius = 8) {
				return control.pickMultipleObjects({ ...point, radius, depth: 20 }).flatMap((info) => {
					const vehicle = info.object as ActiveVehicle | undefined;
					return vehicle?.tripId ? [vehicle] : [];
				});
			}
		});
		return () => {
			onpicker(null);
			if (map.hasControl(control)) map.removeControl(control);
		};
	});
	$effect(() => {
		overlay?.setProps({ layers });
	});
</script>
