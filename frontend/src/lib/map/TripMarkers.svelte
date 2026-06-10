<script lang="ts">
	import { type ComponentProps, onDestroy, untrack } from 'svelte';

	import { SvelteMap } from 'svelte/reactivity';

	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import {
		type ActiveVehicle,
		type RenderUnitTable,
		buildActiveVehiclesAtTime,
		normalizeBearingForIcon,
		renderUnitTableFromIPC
	} from '$lib/map/trajectoryArrow';
	import { VEHICLE_ICON_ATLAS, VEHICLE_ICON_MAPPING } from '$lib/map/vehicleIcons';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import type { PickingInfo } from '@deck.gl/core';
	import { IconLayer } from '@deck.gl/layers';
	import { DeckGLOverlay } from '@svelte-maplibre-gl/deckgl';

	let {
		sources = ['mta_subway'],
		refreshInterval = 30_000,
		enabled = true,
		cursor = $bindable(),
		hoveredTripId = $bindable(null),
		hoveredObject = $bindable(null),
		hoverX = $bindable(0),
		hoverY = $bindable(0)
	}: {
		sources?: Source[];
		routeIds?: string[];
		refreshInterval?: number;
		enabled?: boolean;
		cursor?: 'default' | 'pointer' | undefined;
		hoveredTripId?: string | null;
		hoveredObject?: ActiveVehicle | null;
		hoverX?: number;
		hoverY?: number;
	} = $props();

	const fixedAt = $derived.by(() => {
		const atParam = page.url.searchParams.get('at');
		if (!atParam) return null;
		const parsed = Number(atParam);
		return Number.isFinite(parsed) ? parsed : null;
	});

	const tripResources = trip_context.get();

	let renderUnitsBySource = new SvelteMap<Source, RenderUnitTable>();
	let activeVehicles: ActiveVehicle[] = [];
	let activeVehicleCount = $state(0);
	let frameVersion = $state(0);
	let animationId: number | undefined;
	let fetchTimer: ReturnType<typeof setInterval> | undefined;

	const deckLayers = $derived.by(() => {
		if (!enabled || activeVehicleCount === 0) return [];

		const railVehicles = activeVehicles.filter((vehicle) => isRailVehicle(vehicle));
		const busVehicles = activeVehicles.filter((vehicle) => !isRailVehicle(vehicle));

		return [
			new IconLayer<ActiveVehicle>({
				id: `trip-markers-rail-outline`,
				data: railVehicles,
				iconAtlas: VEHICLE_ICON_ATLAS,
				iconMapping: VEHICLE_ICON_MAPPING,
				getIcon: (vehicle) => vehicle.iconKey,
				getPosition: (vehicle) => vehicle.position,
				getColor: (vehicle) => getVehicleOutlineColor(vehicle),
				getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
				getSize: (vehicle) => {
					const baseSize = vehicle.lengthM * 1.18;
					return vehicle.tripId === hoveredTripId ? baseSize * 1.3 : baseSize;
				},
				sizeUnits: 'meters',
				billboard: false,
				alphaCutoff: 0,
				sizeMinPixels: 9,
				pickable: false,
				updateTriggers: {
					getPosition: frameVersion,
					getAngle: frameVersion,
					getColor: frameVersion,
					getSize: [frameVersion, hoveredTripId],
					getIcon: frameVersion
				}
			}),
			new IconLayer<ActiveVehicle>({
				id: `trip-markers-rail-fill`,
				data: railVehicles,
				iconAtlas: VEHICLE_ICON_ATLAS,
				iconMapping: VEHICLE_ICON_MAPPING,
				getIcon: (vehicle) => vehicle.iconKey,
				getPosition: (vehicle) => vehicle.position,
				getColor: (vehicle) => getVehicleFillColor(vehicle),
				getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
				getSize: (vehicle) => {
					const baseSize = vehicle.lengthM * 0.94;
					return vehicle.tripId === hoveredTripId ? baseSize * 1.3 : baseSize;
				},
				sizeUnits: 'meters',
				billboard: false,
				alphaCutoff: 0,
				sizeMinPixels: 7,
				pickable: true,
				updateTriggers: {
					getPosition: frameVersion,
					getAngle: frameVersion,
					getColor: frameVersion,
					getSize: [frameVersion, hoveredTripId],
					getIcon: frameVersion
				}
			}),
			new IconLayer<ActiveVehicle>({
				id: `trip-markers-bus`,
				data: busVehicles,
				iconAtlas: VEHICLE_ICON_ATLAS,
				iconMapping: VEHICLE_ICON_MAPPING,
				getIcon: (vehicle) => vehicle.iconKey,
				getPosition: (vehicle) => vehicle.position,
				getColor: (vehicle) => vehicle.color,
				getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
				getSize: (vehicle) => {
					const baseSize = vehicle.lengthM;
					return vehicle.tripId === hoveredTripId ? baseSize * 1.3 : baseSize;
				},
				sizeUnits: 'meters',
				billboard: false,
				alphaCutoff: 0,
				sizeMinPixels: 8,
				pickable: true,
				updateTriggers: {
					getPosition: frameVersion,
					getAngle: frameVersion,
					getColor: frameVersion,
					getSize: [frameVersion, hoveredTripId],
					getIcon: frameVersion
				}
			})
		];
	});

	function isRailVehicle(vehicle: ActiveVehicle) {
		return vehicle.iconKey === 'rail_head' || vehicle.iconKey === 'rail_car';
	}

	function getVehicleFillColor(vehicle: ActiveVehicle): [number, number, number, number] {
		if (isRailVehicle(vehicle)) {
			return vehicle.isHead ? [248, 248, 244, 245] : [228, 230, 232, 240];
		}

		return vehicle.color;
	}

	function getVehicleOutlineColor(vehicle: ActiveVehicle): [number, number, number, number] {
		return [vehicle.color[0], vehicle.color[1], vehicle.color[2], 255];
	}

	function clearActiveVehicles() {
		activeVehicles.length = 0;
		activeVehicleCount = 0;
		frameVersion += 1;
	}

	function updateAllVehiclesAtTime(t: number) {
		let combined: ActiveVehicle[] = [];
		for (const src of sources) {
			const table = renderUnitsBySource.get(src);
			if (table) {
				const vehicles: ActiveVehicle[] = [];
				buildActiveVehiclesAtTime(table, t, vehicles);
				combined.push(...vehicles);
			}
		}
		activeVehicles = combined;
		activeVehicleCount = combined.length;
		frameVersion += 1;
	}

	async function openTripModal(tripId: string) {
		const vehicle = activeVehicles.find((v) => v.tripId === tripId);
		if (!vehicle) return;
		const source = vehicle.source as Source;
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
		const tripId = info.object?.tripId;
		if (!tripId) return;

		event.preventDefault();
		event.stopPropagation();

		void openTripModal(tripId);
	}

	function handleDeckHover(info: PickingInfo) {
		const tripId = info.object?.tripId;
		if (tripId) {
			cursor = 'pointer';
			hoveredTripId = tripId;
			hoveredObject = info.object as ActiveVehicle;
			hoverX = info.x;
			hoverY = info.y;
		} else {
			if (hoveredTripId && sources.includes(hoveredObject?.source as Source)) {
				if (cursor === 'pointer') {
					cursor = 'default';
				}
				hoveredTripId = null;
				hoveredObject = null;
			}
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
				}
			}

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
