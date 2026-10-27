<script lang="ts">
	import Icon from '#lib/Route/Icon.svelte';
	import { source_config } from '#lib/sources.js';
	import {
		boardingDistance,
		type BoardingOption,
		type DepartureOption
	} from '#lib/transit/nearby.js';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';

	let {
		option,
		departure,
		show_stop = true,
		time_format = 'countdown',
		arrival,
		show_source = false,
		compact = false
	}: {
		option: BoardingOption;
		departure?: DepartureOption;
		show_stop?: boolean;
		time_format?: 'time' | 'countdown';
		arrival?: number;
		show_source?: boolean;
		compact?: boolean;
	} = $props();
	const current_time = getCurrentTime();
	const boarding = $derived(departure?.boarding ?? option.stops[0]);
	const distance = $derived(boardingDistance(boarding.distance_m, boarding.same_stop));
	const destination = $derived(departure?.destination ?? option.destination);
	const gap = $derived(
		departure?.call && arrival !== undefined
			? Math.floor((departure.call.departure - arrival) / 60_000)
			: undefined
	);
	const time = $derived(
		departure?.call
			? time_format === 'time'
				? new Date(departure.call.departure).toLocaleTimeString([], {
						hour: 'numeric',
						minute: '2-digit'
					})
				: `${Math.max(0, Math.ceil((departure.call.departure - current_time.ms) / 60_000))}m`
			: undefined
	);
	const label = $derived(
		`${option.route.short_name} to ${destination}, ${source_config[option.route.data.source].name}, ${boarding.stop.name}, ${distance}${time ? `, departs ${time}, ${gap} minutes after arrival` : ''}`
	);
	function open() {
		const source = option.route.data.source;
		if (departure?.call)
			open_modal({ type: 'trip', source, route_id: option.route.id, id: departure.call.trip_id });
		else open_modal({ type: 'stop', source, id: boarding.stop.id });
	}
</script>

<button type="button" class={['option', { compact }]} aria-label={label} onclick={open}>
	<span class="badge"><Icon route={option.route} link={false} width={24} height={24} /></span>
	<span class="copy"
		><span class="destination"
			>{#if show_source}{source_config[option.route.data.source].name} ·
			{/if}{destination}</span
		>
		{#if show_stop}<span class="place"
				><span class="stop-name">{boarding.stop.name}</span>{#if departure && !boarding.same_stop}
					<span class="separator" aria-hidden="true">·</span><span class="distance">{distance}</span
					>{/if}</span
			>{/if}
	</span>
	{#if departure}
		<span class="metric">
			{#if time}<span class="time">{time}</span>{#if !compact}<span class="secondary"
						>+{gap} min</span
					>{/if}
			{:else if departure.loading && !departure.error}<span
					class="skeleton animate-pulse"
					aria-label="Loading departures"
				></span>
			{:else}<span class="secondary"
					>{departure.error ? 'Unavailable' : 'No later predictions'}</span
				>{/if}
		</span>
	{:else}<span class="metric">{distance}</span>{/if}
</button>

<style>
	.option {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		min-height: 48px;
		padding: 6px;
		text-align: left;
	}
	.option.compact {
		min-height: 36px;
		padding: 3px 8px 3px 0;
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
		flex: 0 0 42px;
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
		font-size: 14px;
		font-weight: 500;
	}
	.place {
		display: flex;
		align-items: baseline;
		gap: 6px;
		margin-top: 2px;
		font-size: 12px;
		color: var(--text-muted);
	}
	.stop-name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.compact .badge {
		flex-basis: 24px;
	}
	.compact .destination {
		font-size: 13px;
	}
	.compact .place {
		font-size: 11px;
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
	.secondary {
		display: block;
		color: var(--text-muted);
		font-size: 11px;
	}
	.skeleton {
		width: 48px;
		height: 16px;
		background: var(--border-default);
		border-radius: 0;
	}
</style>
