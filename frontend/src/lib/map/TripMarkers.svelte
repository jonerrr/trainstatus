<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';

	import { SvelteMap } from 'svelte/reactivity';

	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import type { MapHover } from '$lib/map/hover.svelte';
	import type { VehiclePicker } from '$lib/map/interactions';
	import { BODY_HEAD_RGB, BODY_RGB, CASING_RGB } from '$lib/map/mapTheme';
	import {
		type ActiveVehicle,
		type RenderUnitTable,
		buildActiveVehiclesAtTime,
		normalizeBearingForIcon,
		renderUnitTableFromIPC
	} from '$lib/map/trajectoryArrow';
	import {
		VEHICLE_ICON_ATLAS,
		VEHICLE_ICON_MAPPING,
		type VehicleIconRole,
		shapeForIconKey
	} from '$lib/map/vehicleIcons';

	import type { PickingInfo } from '@deck.gl/core';
	import { IconLayer } from '@deck.gl/layers';
	import { MapboxOverlay } from '@deck.gl/mapbox';
	import maplibregl from 'maplibre-gl';
	import { getMapContext } from 'svelte-maplibre-gl';

	let {
		sources = ['mta_subway'],
		hover,
		refreshInterval = 30_000,
		enabled = true,
		paused = false,
		railDetail = false,
		busDetail = false,
		pixelRatio = 1,
		onPickerReady
	}: {
		sources?: Source[];
		hover: MapHover;
		refreshInterval?: number;
		enabled?: boolean;
		paused?: boolean;
		railDetail?: boolean;
		busDetail?: boolean;
		pixelRatio?: number;
		onPickerReady?: (picker: VehiclePicker | null) => void;
	} = $props();

	/**
	 * Each vehicle has two size-matched representations. The parent selects the
	 * detail mode only after a zoom gesture settles, avoiding deck attribute
	 * invalidation for every fractional zoom value.
	 *
	 *  - A constant-pixel "puck": one per *trip*, with a directional nose.
	 *  - The true-scale form: `sizeUnits: 'meters'`, one icon per render unit,
	 *    which for a subway trip means one per car.
	 *
	 * This is what fixes the low-zoom smear. Previously the true-scale form was
	 * used at every zoom with `sizeMinPixels: 7`, so at z12 — where an 18.3m car
	 * is half a pixel — all ten cars of a consist clamped to the pixel floor and
	 * piled up on top of each other.
	 *
	 * A 10-car consist is ~183m, which matches the 22px rail puck at about z14.
	 * A 12m bus matches its 15px puck much later, at about z16.7.
	 */
	const RAIL_PUCK_PX = 22;
	const BUS_PUCK_PX = 15;
	const FRAME_INTERVAL_MS = 1000 / 30;

	/** Alpha applied to vehicles that are not the hovered one. */
	const DIMMED = 0.5;
	const HOVER_SCALE = 1.15;

	const fixedAt = $derived.by(() => {
		const atParam = page.url.searchParams.get('at');
		if (!atParam) return null;
		const parsed = Number(atParam);
		return Number.isFinite(parsed) ? parsed : null;
	});

	let renderUnitsBySource = new SvelteMap<Source, RenderUnitTable>();

	/**
	 * Per-source pools of live vehicle objects. `buildActiveVehiclesAtTime`
	 * reuses the objects in its target array, so a pool keeps stable object *and*
	 * array identity across frames while positions mutate in place.
	 *
	 * That stability is what lets `updateTriggers` do its job: deck.gl invalidates
	 * every attribute whenever the `data` prop changes by reference, so handing it
	 * a freshly allocated array each frame (as this component used to) meant colors,
	 * icons and sizes were regenerated 60x/second along with positions. Now only
	 * position and angle are per-frame.
	 *
	 * A SvelteMap is safe here because animation frames mutate pooled vehicle
	 * objects, not the map itself. Its structure changes only when sources change.
	 */
	const vehiclePools = new SvelteMap<Source, ActiveVehicle[]>();

	let activeVehicleCount = $state(0);
	/** Bumped every animation frame — positions and bearings moved. */
	let frameVersion = $state(0);
	/** Bumped only when a new Arrow table lands — the vehicle *set* changed. */
	let dataVersion = $state(0);
	let animationId: number | undefined;
	let fetchTimer: ReturnType<typeof setInterval> | undefined;
	let lastAnimationAt = 0;

	function isRailVehicle(vehicle: ActiveVehicle) {
		return vehicle.iconKey === 'rail_head' || vehicle.iconKey === 'rail_car';
	}

	/** One puck per trip: rail consists collapse onto their head car. */
	function isLeadUnit(vehicle: ActiveVehicle) {
		return !isRailVehicle(vehicle) || vehicle.isHead;
	}

	/**
	 * Which vehicles are drawn in which representation. Rebuilt only when the
	 * vehicle set or one of the two boolean detail modes changes — never per
	 * animation frame, since the pooled objects mutate in place.
	 */
	const partitions = $derived.by(() => {
		// Tracked dependencies: a new table, detail mode, or source toggle.
		const version = dataVersion;
		const currentSources = sources;
		const currentRailDetail = railDetail;
		const currentBusDetail = busDetail;

		const detail: ActiveVehicle[] = [];
		const puck: ActiveVehicle[] = [];

		for (const source of currentSources) {
			const pool = vehiclePools.get(source);
			if (!pool) continue;

			for (const vehicle of pool) {
				const detailed = isRailVehicle(vehicle) ? currentRailDetail : currentBusDetail;
				if (detailed) detail.push(vehicle);
				else if (isLeadUnit(vehicle)) puck.push(vehicle);
			}
		}

		return { detail, puck, version };
	});

	function alphaFor(vehicle: ActiveVehicle, base: number) {
		const dim = hover.tripId && vehicle.tripId !== hover.tripId ? DIMMED : 1;
		return Math.round(base * dim);
	}

	function bodyColor(vehicle: ActiveVehicle): [number, number, number, number] {
		// Buses keep a light body too: filling them with the route colour made them
		// disappear into the identically coloured route line they sit on.
		const rgb = vehicle.isHead ? BODY_HEAD_RGB : BODY_RGB;
		return [rgb[0], rgb[1], rgb[2], alphaFor(vehicle, 245)];
	}

	function ringColor(vehicle: ActiveVehicle): [number, number, number, number] {
		return [vehicle.color[0], vehicle.color[1], vehicle.color[2], alphaFor(vehicle, 255)];
	}

	function casingColor(vehicle: ActiveVehicle): [number, number, number, number] {
		return [CASING_RGB[0], CASING_RGB[1], CASING_RGB[2], alphaFor(vehicle, 235)];
	}

	function colorFor(
		vehicle: ActiveVehicle,
		role: VehicleIconRole
	): [number, number, number, number] {
		if (role === 'casing') return casingColor(vehicle);
		if (role === 'ring') return ringColor(vehicle);
		return bodyColor(vehicle);
	}

	/**
	 * The casing / ring / body passes are three concentric silhouettes of the same
	 * shape drawn at the same size and anchor, so the outline thickness is a fixed
	 * proportion of the icon. The previous approach — one shape at 1.18x under a
	 * copy at 0.94x — collapsed to a 1px fringe as soon as both hit the min-pixel
	 * clamp, which is a large part of why the markers looked muddy.
	 */
	function iconLayers(
		idPrefix: string,
		data: ActiveVehicle[],
		getSize: (vehicle: ActiveVehicle) => number,
		sizeUnits: 'meters' | 'pixels',
		puckShape: boolean
	) {
		return (['casing', 'ring', 'body'] as const).map(
			(role) =>
				new IconLayer<ActiveVehicle>({
					id: `${idPrefix}-${role}`,
					data,
					iconAtlas: VEHICLE_ICON_ATLAS,
					iconMapping: VEHICLE_ICON_MAPPING,
					getIcon: (vehicle) => `${puckShape ? 'puck' : shapeForIconKey(vehicle.iconKey)}_${role}`,
					getPosition: (vehicle) => vehicle.position,
					getColor: (vehicle) => colorFor(vehicle, role),
					getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
					getSize,
					sizeUnits,
					billboard: false,
					alphaCutoff: 0,
					sizeMinPixels: sizeUnits === 'meters' ? 3 : 0,
					// Only the topmost pass is pickable, so a hover produces one hit
					// rather than three stacked ones.
					pickable: role === 'body',
					updateTriggers: {
						getPosition: frameVersion,
						getAngle: frameVersion,
						getIcon: dataVersion,
						getColor: [dataVersion, hover.tripId],
						getSize: [dataVersion, hover.tripId]
					}
				})
		);
	}

	const deckLayers = $derived.by(() => {
		if (!enabled || activeVehicleCount === 0) return [];

		const { detail, puck } = partitions;
		const layers: IconLayer<ActiveVehicle>[] = [];

		if (puck.length > 0) {
			layers.push(
				...iconLayers(
					'trip-markers-puck',
					puck,
					(vehicle) => {
						const base = isRailVehicle(vehicle) ? RAIL_PUCK_PX : BUS_PUCK_PX;
						return vehicle.tripId === hover.tripId ? base * HOVER_SCALE : base;
					},
					'pixels',
					true
				)
			);
		}

		if (detail.length > 0) {
			layers.push(
				...iconLayers(
					'trip-markers-detail',
					detail,
					(vehicle) =>
						vehicle.tripId === hover.tripId ? vehicle.lengthM * HOVER_SCALE : vehicle.lengthM,
					'meters',
					false
				)
			);
		}

		return layers;
	});

	function clearActiveVehicles() {
		vehiclePools.clear();
		activeVehicleCount = 0;
		dataVersion += 1;
		frameVersion += 1;
	}

	function updateAllVehiclesAtTime(t: number) {
		let count = 0;

		for (const source of sources) {
			const table = renderUnitsBySource.get(source);
			if (!table) continue;

			let pool = vehiclePools.get(source);
			if (!pool) {
				pool = [];
				vehiclePools.set(source, pool);
			}

			try {
				// Mutates the pooled objects in place, preserving array identity.
				buildActiveVehiclesAtTime(table, t, pool);
			} catch (err) {
				// This runs inside the requestAnimationFrame loop, so an unguarded
				// throw here freezes every vehicle on the map until the component
				// remounts. A truncated Arrow payload — e.g. the backend dying
				// mid-response — parses fine but blows up on access, so drop the
				// unusable table and let the next refresh replace it.
				console.error(`[TripMarkers] dropping unusable trajectory data for ${source}:`, err);
				renderUnitsBySource.delete(source);
				vehiclePools.delete(source);
				dataVersion += 1;
				continue;
			}

			count += pool.length;
		}

		activeVehicleCount = count;
		frameVersion += 1;
	}

	/**
	 * Pooled objects are reused, so after a refetch the object a hover points at
	 * may describe a different vehicle. Re-resolve it by id, or drop it.
	 */
	function reconcileHover() {
		const hovered = hover.vehicle;
		if (!hovered) return;

		for (const source of sources) {
			for (const vehicle of vehiclePools.get(source) ?? []) {
				if (vehicle.renderUnitId === hovered.vehicle.renderUnitId) {
					hover.vehicle = { ...hovered, vehicle };
					return;
				}
			}
		}

		hover.setVehicle(null);
	}

	function handleDeckHover(info: PickingInfo) {
		if (paused) {
			hover.setVehicle(null);
			return;
		}
		const vehicle = info.object as ActiveVehicle | undefined;

		if (vehicle?.tripId) {
			hover.setVehicle({
				kind: 'vehicle',
				routeId: vehicle.routeId,
				source: vehicle.source,
				tripId: vehicle.tripId,
				vehicle,
				x: info.x,
				y: info.y
			});
		} else if (hover.vehicle) {
			hover.setVehicle(null);
		}
	}

	async function fetchTrajectories(
		currentSource: Source,
		currentFixedAt: number | null,
		currentEnabled: boolean
	) {
		if (!currentEnabled) {
			return;
		}

		try {
			const params = new URLSearchParams();
			if (currentFixedAt !== null) params.set('at', String(currentFixedAt));

			const queryString = params.toString();
			const url = `/api/v1/trajectories/${currentSource}${queryString ? `?${queryString}` : ''}`;

			const res = await fetch(url);
			if (!res.ok) {
				console.error(`Failed to fetch trajectories for ${currentSource}: ${res.status}`);
				return;
			}

			const nextRenderUnits = renderUnitTableFromIPC(await res.arrayBuffer());
			renderUnitsBySource.set(currentSource, nextRenderUnits);
			updateAllVehiclesAtTime(currentFixedAt ?? Date.now() / 1000);
			dataVersion += 1;
			reconcileHover();

			if (nextRenderUnits.table.numRows > 0 && activeVehicleCount === 0) {
				console.warn(
					`[TripMarkers] ${nextRenderUnits.table.numRows} render units loaded but none were active at t=${currentFixedAt ?? Date.now() / 1000}`
				);
			}
		} catch (err) {
			console.error(`Error fetching trajectories for ${currentSource}:`, err);
		}
	}

	let isRunning = false;

	function animate(now: number) {
		if (!enabled || paused || renderUnitsBySource.size === 0 || fixedAt !== null) {
			isRunning = false;
			animationId = undefined;
			return;
		}
		if (now - lastAnimationAt >= FRAME_INTERVAL_MS) {
			updateAllVehiclesAtTime(Date.now() / 1000);
			lastAnimationAt = now;
		}
		animationId = requestAnimationFrame(animate);
	}

	function startAnimation() {
		if (!isRunning && enabled && !paused && renderUnitsBySource.size > 0 && fixedAt === null) {
			isRunning = true;
			lastAnimationAt = performance.now();
			animationId = requestAnimationFrame(animate);
		}
	}

	function stopAnimation() {
		if (isRunning) {
			isRunning = false;
			if (animationId !== undefined) {
				cancelAnimationFrame(animationId);
				animationId = undefined;
			}
		}
	}

	$effect(() => {
		const currentEnabled = enabled;
		const currentPaused = paused;
		const hasRenderUnits = renderUnitsBySource.size > 0;
		const currentFixedAt = fixedAt;

		if (currentEnabled && !currentPaused && hasRenderUnits && currentFixedAt === null) {
			untrack(() => updateAllVehiclesAtTime(Date.now() / 1000));
			startAnimation();
		} else {
			stopAnimation();
		}
	});

	$effect(() => {
		const currentFixedAt = fixedAt;
		if (renderUnitsBySource.size > 0) {
			untrack(() => {
				updateAllVehiclesAtTime(currentFixedAt ?? Date.now() / 1000);
			});
		}
	});

	$effect(() => {
		const currentEnabled = enabled;
		const currentSources = sources;
		const currentFixedAt = fixedAt;
		untrack(() => {
			if (!currentEnabled) {
				renderUnitsBySource.clear();
				clearActiveVehicles();
				return;
			}

			// Remove keys for sources that are no longer active
			for (const key of Array.from(renderUnitsBySource.keys())) {
				if (!currentSources.includes(key)) {
					renderUnitsBySource.delete(key);
					vehiclePools.delete(key);
				}
			}

			// A hover latched onto a de-selected source can never clear itself,
			// because its layer stops receiving mouse events.
			hover.pruneSources(currentSources);
			dataVersion += 1;

			// Fetch trajectories for active sources
			for (const src of currentSources) {
				void fetchTrajectories(src, currentFixedAt, currentEnabled);
			}
		});
	});

	$effect(() => {
		const currentEnabled = enabled;
		const currentSources = sources;
		const currentFixedAt = fixedAt;
		const currentRefreshInterval = refreshInterval;

		if (fetchTimer !== undefined) {
			clearInterval(fetchTimer);
			fetchTimer = undefined;
		}

		if (!currentEnabled || currentFixedAt !== null) return;

		fetchTimer = setInterval(() => {
			for (const src of currentSources) {
				void fetchTrajectories(src, null, currentEnabled);
			}
		}, currentRefreshInterval);

		return () => {
			if (fetchTimer !== undefined) {
				clearInterval(fetchTimer);
				fetchTimer = undefined;
			}
		};
	});
	// TODO: why not use deckgloverlay component from maplibre svelte?
	// if not, then maybe make a copy with the fixes we need and use that to clean up the code
	const mapCtx = getMapContext();
	if (!mapCtx.map) throw new Error('Map instance is not initialized.');

	let deckOverlay = $state<MapboxOverlay>();
	const picker: VehiclePicker = {
		pick(point, radius = 8) {
			if (!deckOverlay || !enabled) return [];

			const vehicles: ActiveVehicle[] = [];
			for (const info of deckOverlay.pickMultipleObjects({ ...point, radius, depth: 20 })) {
				const vehicle = info.object as ActiveVehicle | undefined;
				if (!vehicle?.tripId) continue;
				const key = `${vehicle.source}:${vehicle.tripId}`;
				if (vehicles.some((candidate) => `${candidate.source}:${candidate.tripId}` === key))
					continue;
				vehicles.push(vehicle);
			}
			return vehicles;
		}
	};

	onMount(() => {
		deckOverlay = new MapboxOverlay({
			interleaved: true,
			layers: deckLayers,
			onHover: handleDeckHover,
			useDevicePixels: pixelRatio,
			_pickable: enabled && !paused
		});
		mapCtx.map?.addControl(deckOverlay as maplibregl.IControl);
		onPickerReady?.(picker);
	});

	$effect(() => {
		const overlay = deckOverlay;
		if (!overlay) return;
		overlay.setProps({
			layers: deckLayers,
			onHover: handleDeckHover,
			useDevicePixels: pixelRatio,
			_pickable: enabled && !paused
		});
	});

	onDestroy(() => {
		stopAnimation();
		if (fetchTimer !== undefined) clearInterval(fetchTimer);
		onPickerReady?.(null);
		if (deckOverlay && mapCtx.map?.hasControl(deckOverlay as maplibregl.IControl)) {
			mapCtx.map.removeControl(deckOverlay as maplibregl.IControl);
		}
	});
</script>
