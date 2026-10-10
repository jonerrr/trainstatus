<script lang="ts">
	import { untrack } from 'svelte';

	import { SvelteMap } from 'svelte/reactivity';

	import type { Source } from '#lib/client/index.js';
	import { getUpdateStatus } from '#lib/resources/status.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	import DeckOverlay from './DeckOverlay.svelte';
	import type { MapFeatureKey, VehiclePicker } from './interactions';
	import { watchTrajectories, type TrajectorySnapshot } from './trajectories';
	import { buildActiveVehiclesAtTime, type ActiveVehicle } from './trajectoryArrow';
	import { isRailVehicle, vehicleLayers } from './vehicleLayers';

	const current_time = getCurrentTime();

	let {
		sources,
		railDetail = false,
		busDetail = false,
		selectedTrip = null,
		onPickerReady
	}: {
		sources: Source[];
		railDetail?: boolean;
		busDetail?: boolean;
		selectedTrip?: MapFeatureKey | null;
		onPickerReady: (picker: VehiclePicker | null) => void;
	} = $props();
	const fixedAt = $derived(current_time.value ?? null);
	let snapshot = $state.raw<TrajectorySnapshot>({
		tables: new Map(),
		statuses: new Map()
	});
	const identities = new WeakMap<ActiveVehicle[], string[]>();
	const pools = new SvelteMap<Source, ActiveVehicle[]>();
	let frame = $state(0);
	let membership = $state(0);

	const updates = getUpdateStatus();
	$effect(() => {
		const selected = [...sources];
		const at = fixedAt;
		return untrack(() => {
			return watchTrajectories(
				{ sources: selected, at, refreshInterval: 30_000 },
				(next) => {
					snapshot = next;
				},
				undefined,
				updates
			);
		});
	});

	function update(time: number) {
		let changed = false;
		for (const source of pools.keys()) {
			if (!snapshot.tables.has(source)) {
				pools.delete(source);
				changed = true;
			}
		}
		for (const [source, table] of snapshot.tables) {
			const pool = pools.get(source) ?? [];
			const previousIds = identities.get(pool) ?? [];
			identities.set(pool, previousIds);
			buildActiveVehiclesAtTime(table, time, pool);
			// Reuse the identity buffer rather than allocating arrays/strings every frame.
			if (previousIds.length !== pool.length) changed = true;
			for (let i = 0; i < pool.length; i++) {
				if (previousIds[i] !== pool[i].renderUnitId) changed = true;
				previousIds[i] = pool[i].renderUnitId;
			}
			previousIds.length = pool.length;
			pools.set(source, pool);
		}
		if (changed) membership++;
		frame++;
	}

	$effect(() => {
		void snapshot;
		const at = fixedAt;
		untrack(() => {
			update(at ?? Date.now() / 1000);
			membership++;
		});
		if (at !== null) return;
		let previous = 0;
		let id: number;
		function animate(now: number) {
			if (now - previous >= 1000 / 30) {
				update(Date.now() / 1000);
				previous = now;
			}
			id = requestAnimationFrame(animate);
		}
		id = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(id);
	});
	const partitions = $derived.by(() => {
		void membership;
		const detail: ActiveVehicle[] = [];
		const puck: ActiveVehicle[] = [];
		for (const source of sources)
			for (const vehicle of pools.get(source) ?? []) {
				if (isRailVehicle(vehicle) ? railDetail : busDetail) detail.push(vehicle);
				else if (!isRailVehicle(vehicle) || vehicle.isHead) puck.push(vehicle);
			}
		return { detail, puck };
	});
	const layers = $derived(vehicleLayers(partitions, selectedTrip, frame, membership));
</script>

<DeckOverlay {layers} onpicker={onPickerReady} />
