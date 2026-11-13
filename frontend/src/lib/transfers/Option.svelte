<script lang="ts">
	import { boardingDistance, type DepartureOption } from '#lib/data/nearby.js';
	import Icon from '#lib/route/Icon.svelte';
	import { sourceConfig } from '#lib/sources.js';
	import { getCurrentTime, openModal } from '#lib/url-params.svelte.js';

	let {
		departure,
		showStop = true,
		timeFormat = 'countdown',
		arrival,
		showSource = false,
		target = 'trip'
	}: {
		departure: DepartureOption;
		showStop?: boolean;
		timeFormat?: 'time' | 'countdown';
		arrival?: number;
		showSource?: boolean;
		target?: 'stop' | 'trip';
	} = $props();
	const currentTime = getCurrentTime();
	const boarding = $derived(departure.boarding);
	const distance = $derived(boardingDistance(boarding.distanceM, boarding.sameStop));
	const showDistance = $derived(!boarding.sameStop);
	const destination = $derived(departure.destination);
	const gap = $derived(
		arrival !== undefined ? Math.floor((departure.call.departure - arrival) / 60_000) : undefined
	);
	const time = $derived(
		timeFormat === 'time'
			? new Date(departure.call.departure).toLocaleTimeString([], {
					hour: 'numeric',
					minute: '2-digit'
				})
			: `${Math.max(0, Math.ceil((departure.call.departure - currentTime.ms) / 60_000))}m`
	);
	const label = $derived(
		`${departure.route.short_name} to ${destination}, ${sourceConfig[departure.route.data.source].name}, ${boarding.stop.name}, ${distance}, departs ${time}${gap !== undefined ? `, ${gap} minutes after arrival` : ''}`
	);
	function open() {
		const source = departure.route.data.source;
		if (target === 'trip')
			openModal({
				type: 'trip',
				source,
				routeId: departure.route.id,
				id: departure.call.tripId
			});
		else openModal({ type: 'stop', source, id: boarding.stop.id });
	}
</script>

<button type="button" class="option" aria-label={label} onclick={open}>
	<span class="badge"><Icon route={departure.route} link={false} width={24} height={24} /></span>
	<span class="copy"
		><span class="destination"
			>{#if showSource}{sourceConfig[departure.route.data.source].name} ·
			{/if}{destination}</span
		>
		{#if showStop || showDistance}<span class="place"
				>{#if showStop}<span class="stop-name">{boarding.stop.name}</span>{/if}{#if showDistance}
					{#if showStop}<span class="separator" aria-hidden="true">·</span>{/if}<span
						class="distance">{distance}</span
					>{/if}</span
			>{/if}
	</span>
	<span class="metric">
		<span class="time">{time}</span>
	</span>
</button>

<style>
	.option {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		min-height: 36px;
		padding: 3px 8px 3px 0;
		text-align: left;
	}
	:global(.option) + .option {
		border-top: 1px solid var(--bg-hover);
	}
	.option:hover {
		background: var(--bg-hover);
	}
	.option:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
	.badge {
		display: flex;
		justify-content: center;
		flex: 0 0 24px;
	}
	.copy {
		min-width: 0;
		flex: 1;
	}
	.destination,
	.place {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.destination {
		font-size: 13px;
		font-weight: 500;
	}
	.place {
		display: flex;
		align-items: baseline;
		gap: 6px;
		margin-top: 2px;
		font-size: 11px;
		color: var(--text-muted);
	}
	.stop-name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.separator,
	.distance {
		flex-shrink: 0;
	}
	.metric {
		flex: 0 0 auto;
		max-width: 84px;
		min-height: 32px;
		display: flex;
		flex-direction: column;
		justify-content: center;
		text-align: right;
		font-size: 12px;
		font-variant-numeric: tabular-nums;
	}
	.time {
		font-size: 14px;
		white-space: nowrap;
	}
</style>
