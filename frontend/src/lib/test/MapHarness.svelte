<script lang="ts">
	import { onDestroy } from 'svelte';

	import { setWorkerUrl, type Map } from 'maplibre-gl';
	import sharedCode from 'maplibre-gl/dist/maplibre-gl-shared.mjs?raw';
	import workerCode from 'maplibre-gl/dist/maplibre-gl-worker.mjs?raw';
	import { MapLibre } from 'svelte-maplibre-gl';

	import 'maplibre-gl/dist/maplibre-gl.css';

	import { IconLayer } from '@deck.gl/layers';

	import DeckOverlay from '../map/DeckOverlay.svelte';
	import FeatureChooser from '../map/FeatureChooser.svelte';
	import Filters from '../map/Filters.svelte';
	import { MapFilters } from '../map/filters.svelte';
	import type { VehiclePicker } from '../map/interactions';
	import type { ActiveVehicle } from '../map/trajectoryArrow';
	import { VEHICLE_ICON_ATLAS, VEHICLE_ICON_MAPPING } from '../map/vehicleIcons';

	// Vitest's module instrumentation runs in the window, not worker globals.
	// Load the installed, unmodified MapLibre worker modules as local blobs.
	const sharedUrl = URL.createObjectURL(new Blob([sharedCode], { type: 'text/javascript' }));
	const workerUrl = URL.createObjectURL(
		new Blob([workerCode.replace('./maplibre-gl-shared.mjs', sharedUrl)], {
			type: 'text/javascript'
		})
	);
	setWorkerUrl(workerUrl);
	onDestroy(() => {
		URL.revokeObjectURL(workerUrl);
		URL.revokeObjectURL(sharedUrl);
	});

	let map = $state.raw<Map>();
	let center = $state<[number, number]>([0, 0]);
	let enabled = $state(true);
	let chooser = $state(false);
	let picker: VehiclePicker | null = null;
	let filters = $state(new MapFilters());
	const vehicle: ActiveVehicle = {
		renderUnitId: 'one',
		source: 'mta_subway',
		tripId: 'trip',
		routeId: 'A',
		iconKey: 'rail_head',
		unitIndex: 0,
		unitCount: 1,
		isHead: true,
		lengthM: 20,
		passengers: null,
		color: [0, 100, 200, 255],
		position: [0, 0],
		bearing: 0
	};
	const layers = $derived(
		enabled
			? [
					new IconLayer<ActiveVehicle>({
						id: 'fixture-vehicle',
						data: [vehicle],
						iconAtlas: VEHICLE_ICON_ATLAS,
						iconMapping: VEHICLE_ICON_MAPPING,
						getIcon: () => 'puck_body',
						getPosition: (v) => v.position,
						getSize: 32,
						pickable: true
					})
				]
			: []
	);
	export function read() {
		return { map, picker, center };
	}
</script>

<div
	style="position:relative;width:800px;height:500px"
	data-test-map
	data-center={center.join(',')}
>
	<MapLibre
		bind:map
		bind:center
		style={{
			version: 8,
			sources: {},
			layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#111820' } }]
		}}
		zoom={12}
		class="size-full"
		autoloadGlobalCss={false}
		attributionControl={false}
	>
		{#if enabled}<DeckOverlay
				{layers}
				onpicker={(value) => {
					picker = value;
				}}
			/>{/if}
	</MapLibre>
	<Filters bind:filters />
	{#if chooser}<FeatureChooser
			targets={[{ kind: 'trip', id: 'trip', source: 'mta_subway' }]}
			point={{ x: 20, y: 200 }}
			viewportWidth={800}
			viewportHeight={500}
			onselect={() => {
				chooser = false;
			}}
			ondismiss={() => {
				chooser = false;
			}}
		>
			A train to Downtown
		</FeatureChooser>{/if}
</div>
<button
	onclick={() => {
		enabled = !enabled;
	}}>Toggle vehicles</button
>
<button
	onclick={() => {
		chooser = true;
	}}>Choose features</button
>
