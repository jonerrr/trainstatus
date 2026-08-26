<script lang="ts">
	import { type ComponentProps, onDestroy, untrack } from 'svelte';

	import { SvelteMap } from 'svelte/reactivity';

	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import type { MapHover } from '$lib/map/hover.svelte';
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
	import { trip_context } from '$lib/resources/trips.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import type { PickingInfo } from '@deck.gl/core';
	import { IconLayer } from '@deck.gl/layers';
	import { DeckGLOverlay } from '@svelte-maplibre-gl/deckgl';

	let {
		sources = ['mta_subway'],
		zoom = 12,
		hover,
		refreshInterval = 30_000,
		enabled = true
	}: {
		sources?: Source[];
		zoom?: number;
		hover: MapHover;
		refreshInterval?: number;
		enabled?: boolean;
	} = $props();

	/**
	 * Each vehicle has two representations, and the zoom at which we swap is the
	 * zoom at which they happen to be the same size on screen — so the switch
	 * reads as the shape gaining detail rather than as a pop.
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
	const RAIL_BAND = { lo: 13.6, hi: 14.6 };
	const BUS_BAND = { lo: 16.2, hi: 17.2 };

	const RAIL_PUCK_PX = 22;
	const BUS_PUCK_PX = 15;

	/** Alpha applied to vehicles that are not the hovered one. */
	const DIMMED = 0.5;
	const HOVER_SCALE = 1.15;

	const fixedAt = $derived.by(() => {
		const atParam = page.url.searchParams.get('at');
		if (!atParam) return null;
		const parsed = Number(atParam);
		return Number.isFinite(parsed) ? parsed : null;
	});

	const tripResources = trip_context.get();

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
	 * Deliberately a plain Map, not a SvelteMap: reading it must not make the
	 * partition derivation re-run on every frame.
	 */
	const vehiclePools = new Map<Source, ActiveVehicle[]>();

	let activeVehicleCount = $state(0);
	/** Bumped every animation frame — positions and bearings moved. */
	let frameVersion = $state(0);
	/** Bumped only when a new Arrow table lands — the vehicle *set* changed. */
	let dataVersion = $state(0);
	let animationId: number | undefined;
	let fetchTimer: ReturnType<typeof setInterval> | undefined;

	/** Quantised so a zoom gesture rebuilds colors ~10x per level, not per frame. */
	const zoomKey = $derived(Math.round(zoom * 10));

	function smoothstep(lo: number, hi: number, x: number) {
		const t = Math.min(1, Math.max(0, (x - lo) / (hi - lo)));
		return t * t * (3 - 2 * t);
	}

	function isRailVehicle(vehicle: ActiveVehicle) {
		return vehicle.iconKey === 'rail_head' || vehicle.iconKey === 'rail_car';
	}

	/** 0 = draw as a puck, 1 = draw at true scale, in between = cross-fading. */
	function detailFraction(vehicle: ActiveVehicle, atZoom: number) {
		const band = isRailVehicle(vehicle) ? RAIL_BAND : BUS_BAND;
		return smoothstep(band.lo, band.hi, atZoom);
	}

	/** One puck per trip: rail consists collapse onto their head car. */
	function isLeadUnit(vehicle: ActiveVehicle) {
		return !isRailVehicle(vehicle) || vehicle.isHead;
	}

	/**
	 * Which vehicles are drawn in which representation. Rebuilt only when the
	 * vehicle set changes or the zoom crosses into/through a fade band — never
	 * per frame, since the pooled objects mutate in place.
	 */
	const partitions = $derived.by(() => {
		// Tracked dependencies: a new table, a zoom change, or a source toggle.
		const version = dataVersion;
		const currentZoom = zoom;
		const currentSources = sources;

		const detail: ActiveVehicle[] = [];
		const puck: ActiveVehicle[] = [];

		for (const source of currentSources) {
			const pool = vehiclePools.get(source);
			if (!pool) continue;

			for (const vehicle of pool) {
				const fraction = detailFraction(vehicle, currentZoom);
				if (fraction > 0) detail.push(vehicle);
				if (fraction < 1 && isLeadUnit(vehicle)) puck.push(vehicle);
			}
		}

		return { detail, puck, version };
	});

	function alphaFor(vehicle: ActiveVehicle, base: number, detailPass: boolean) {
		const fraction = detailFraction(vehicle, zoom);
		const fade = detailPass ? fraction : 1 - fraction;
		const dim = hover.tripId && vehicle.tripId !== hover.tripId ? DIMMED : 1;
		return Math.round(base * fade * dim);
	}

	function bodyColor(
		vehicle: ActiveVehicle,
		detailPass: boolean
	): [number, number, number, number] {
		// Buses keep a light body too: filling them with the route colour made them
		// disappear into the identically coloured route line they sit on.
		const rgb = vehicle.isHead ? BODY_HEAD_RGB : BODY_RGB;
		return [rgb[0], rgb[1], rgb[2], alphaFor(vehicle, 245, detailPass)];
	}

	function ringColor(
		vehicle: ActiveVehicle,
		detailPass: boolean
	): [number, number, number, number] {
		return [
			vehicle.color[0],
			vehicle.color[1],
			vehicle.color[2],
			alphaFor(vehicle, 255, detailPass)
		];
	}

	function casingColor(
		vehicle: ActiveVehicle,
		detailPass: boolean
	): [number, number, number, number] {
		return [CASING_RGB[0], CASING_RGB[1], CASING_RGB[2], alphaFor(vehicle, 235, detailPass)];
	}

	function colorFor(
		vehicle: ActiveVehicle,
		role: VehicleIconRole,
		detailPass: boolean
	): [number, number, number, number] {
		if (role === 'casing') return casingColor(vehicle, detailPass);
		if (role === 'ring') return ringColor(vehicle, detailPass);
		return bodyColor(vehicle, detailPass);
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
		detailPass: boolean,
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
					getColor: (vehicle) => colorFor(vehicle, role, detailPass),
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
						getColor: [dataVersion, zoomKey, hover.tripId],
						getSize: [dataVersion, zoomKey, hover.tripId]
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
					false,
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
					true,
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

	async function openTripModal(tripId: string, source: Source) {
		const tripResource = tripResources[source];
		if (!tripResource) return;

		const trip = tripResource.current?.get(tripId);
		if (trip) {
			open_modal({ type: 'trip', ...trip });
			return;
		}

		try {
			const trips = await tripResource.whenReady();
			const readyTrip = trips.get(tripId);
			if (readyTrip) {
				open_modal({ type: 'trip', ...readyTrip });
			}
		} catch (err) {
			console.error('Unable to resolve trip for marker click:', err);
		}
	}

	type DeckGLOnClick = ComponentProps<typeof DeckGLOverlay>['onClick'];
	type DeckGLClickArgs = Parameters<NonNullable<DeckGLOnClick>>;
	type DeckGLClickEvent = DeckGLClickArgs[1];

	function handleDeckClick(info: PickingInfo, event: DeckGLClickEvent) {
		const vehicle = info.object as ActiveVehicle | undefined;
		if (!vehicle?.tripId) return;

		event.preventDefault();
		event.stopPropagation();

		void openTripModal(vehicle.tripId, vehicle.source as Source);
	}

	function handleDeckHover(info: PickingInfo) {
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

	function animate() {
		if (!enabled || renderUnitsBySource.size === 0 || fixedAt !== null) {
			isRunning = false;
			animationId = undefined;
			return;
		}
		updateAllVehiclesAtTime(Date.now() / 1000);
		animationId = requestAnimationFrame(animate);
	}

	function startAnimation() {
		if (!isRunning && enabled && renderUnitsBySource.size > 0 && fixedAt === null) {
			isRunning = true;
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
		const hasRenderUnits = renderUnitsBySource.size > 0;
		const currentFixedAt = fixedAt;

		if (currentEnabled && hasRenderUnits && currentFixedAt === null) {
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

	onDestroy(() => {
		stopAnimation();
		if (fetchTimer !== undefined) clearInterval(fetchTimer);
	});
</script>

{#if enabled}
	<DeckGLOverlay
		interleaved
		layers={deckLayers}
		onClick={handleDeckClick}
		onHover={handleDeckHover}
	/>
{/if}
