<script lang="ts">
	import { onDestroy } from 'svelte';

	import { setWorkerUrl, type Map as MapInstance } from 'maplibre-gl';
	import sharedCode from 'maplibre-gl/dist/maplibre-gl-shared.mjs?raw';
	import workerCode from 'maplibre-gl/dist/maplibre-gl-worker.mjs?raw';
	import { MapLibre } from 'svelte-maplibre-gl';

	import 'maplibre-gl/dist/maplibre-gl.css';

	import type { Source } from '#lib/client/index.js';

	import FeatureChooser from '../map/FeatureChooser.svelte';
	import Filters from '../map/Filters.svelte';
	import { MapFilters } from '../map/filters.svelte';
	import type { VehiclePicker } from '../map/interactions';
	import { renderUnitTableFromIPC, type RenderUnitTable } from '../map/trajectory';
	import { trajectoryFixture } from '../map/trajectoryFixture';
	import TripMarkers from '../map/TripMarkers.svelte';

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

	let map = $state.raw<MapInstance>();
	let center = $state<[number, number]>([0, 0]);
	let enabled = $state(true);
	let chooser = $state(false);
	let picker: VehiclePicker | null = null;
	const filters = new MapFilters();
	let {
		tables = new Map([
			[
				'mta_subway',
				renderUnitTableFromIPC(
					trajectoryFixture(['one'], 200, {
						positions: [
							[-0.001, 0],
							[0.001, 0]
						],
						distances: [0, 200],
						motion: [{ t0: 100, t1: 200, a: 100, b: 0, c: 0, d: 0 }]
					})
				)
			]
		]),
		at,
		railDetail = false,
		busDetail = false
	}: {
		tables?: ReadonlyMap<Source, RenderUnitTable>;
		at?: number;
		railDetail?: boolean;
		busDetail?: boolean;
	} = $props();
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
		canvasContextAttributes={{ preserveDrawingBuffer: true }}
		class="size-full"
		autoloadGlobalCss={false}
		attributionControl={false}
	>
		{#if enabled}<TripMarkers
				{tables}
				{at}
				{railDetail}
				{busDetail}
				onPickerReady={(value) => {
					picker = value;
				}}
			/>{/if}
	</MapLibre>
	<Filters {filters} />
	{#if chooser}<FeatureChooser
			targets={[{ kind: 'trip', id: 'trip', source: 'mta_subway', routeId: 'A' }]}
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
