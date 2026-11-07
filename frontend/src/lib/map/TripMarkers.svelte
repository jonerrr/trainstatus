<script lang="ts">
	import type { Source } from '#lib/client/index.js';

	import { MapLibreOverlay } from '@deck.gl/maplibre';
	import { getMapContext } from 'svelte-maplibre-gl';

	import type { MapFeatureKey, VehiclePicker } from './interactions';
	import {
		buildActiveVehiclesAtTime,
		type ActiveVehicle,
		type RenderUnitTable
	} from './trajectory';
	import { isRailVehicle, vehicleLayers } from './vehicleLayers';

	let {
		tables,
		at,
		railDetail = false,
		busDetail = false,
		selectedTrip = null,
		onPickerReady
	}: {
		tables: ReadonlyMap<Source, RenderUnitTable>;
		at: number | undefined;
		railDetail?: boolean;
		busDetail?: boolean;
		selectedTrip?: MapFeatureKey | null;
		onPickerReady: (picker: VehiclePicker | null) => void;
	} = $props();

	const context = getMapContext();
	let overlay = $state.raw<MapLibreOverlay>();
	$effect(() => {
		const map = context.map;
		if (!map) return;
		const control = new MapLibreOverlay({ interleaved: true });
		map.addControl(control);
		overlay = control;
		onPickerReady({
			pickClosest(point, radius = 8) {
				return (
					(control.pickObject({ ...point, radius })?.object as ActiveVehicle | undefined) ?? null
				);
			},
			pick(point, radius = 8) {
				return control
					.pickMultipleObjects({ ...point, radius, depth: 20 })
					.map((info) => info.object as ActiveVehicle);
			}
		});
		return () => {
			onPickerReady(null);
			if (map.hasControl(control)) map.removeControl(control);
			overlay = undefined;
		};
	});

	$effect(() => {
		const control = overlay;
		const snapshot = tables;
		const time = at;
		const rail = railDetail;
		const bus = busDetail;
		const selection = selectedTrip;
		if (!control) return;

		const vehicles: ActiveVehicle[] = [];
		let detail: ActiveVehicle[] = [];
		let puck: ActiveVehicle[] = [];
		let frame = 0;
		const draw = (time: number) => {
			let detailCount = 0;
			let puckCount = 0;
			let changed = false;
			for (const table of snapshot.values()) {
				buildActiveVehiclesAtTime(table, time, vehicles);
				for (const vehicle of vehicles) {
					if (isRailVehicle(vehicle) ? rail : bus) {
						changed ||= detail[detailCount] !== vehicle;
						detail[detailCount++] = vehicle;
					} else if (!isRailVehicle(vehicle) || vehicle.isHead) {
						changed ||= puck[puckCount] !== vehicle;
						puck[puckCount++] = vehicle;
					}
				}
			}
			changed ||= detail.length !== detailCount || puck.length !== puckCount;
			detail.length = detailCount;
			puck.length = puckCount;
			// A new container invalidates static attributes when membership changes.
			// Stable containers need only position and angle uploads during animation.
			if (changed) {
				detail = detail.slice();
				puck = puck.slice();
			}
			control.setProps({ layers: vehicleLayers({ detail, puck }, selection, ++frame) });
		};

		draw(time ?? Date.now() / 1000);
		if (time !== undefined) return;
		let id: number;
		function animate() {
			draw(Date.now() / 1000);
			id = requestAnimationFrame(animate);
		}
		id = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(id);
	});
</script>
