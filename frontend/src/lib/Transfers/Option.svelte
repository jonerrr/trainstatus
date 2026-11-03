<script lang="ts">
	import Icon from '#lib/Route/Icon.svelte';
	import { source_config } from '#lib/sources.js';
	import { boardingDistance, type DepartureOption } from '#lib/transit/nearby.js';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';

	let {
		departure,
		show_stop = true,
		time_format = 'countdown',
		arrival,
		show_source = false,
		target = 'trip'
	}: {
		departure: DepartureOption;
		show_stop?: boolean;
		time_format?: 'time' | 'countdown';
		arrival?: number;
		show_source?: boolean;
		target?: 'stop' | 'trip';
	} = $props();
	const current_time = getCurrentTime();
	const boarding = $derived(departure.boarding);
	const distance = $derived(boardingDistance(boarding.distance_m, boarding.same_stop));
	const show_distance = $derived(!boarding.same_stop);
	const destination = $derived(departure.destination);
	const gap = $derived(
		arrival !== undefined ? Math.floor((departure.call.departure - arrival) / 60_000) : undefined
	);
	const time = $derived(
		time_format === 'time'
			? new Date(departure.call.departure).toLocaleTimeString([], {
					hour: 'numeric',
					minute: '2-digit'
				})
			: `${Math.max(0, Math.ceil((departure.call.departure - current_time.ms) / 60_000))}m`
	);
	const label = $derived(
		`${departure.route.short_name} to ${destination}, ${source_config[departure.route.data.source].name}, ${boarding.stop.name}, ${distance}, departs ${time}${gap !== undefined ? `, ${gap} minutes after arrival` : ''}`
	);
	function open() {
		const source = departure.route.data.source;
		if (target === 'trip')
			open_modal({
				type: 'trip',
				source,
				route_id: departure.route.id,
				id: departure.call.trip_id
			});
		else open_modal({ type: 'stop', source, id: boarding.stop.id });
	}
</script>

<button type="button" class="option" aria-label={label} onclick={open}>
	<span class="badge"><Icon route={departure.route} link={false} width={24} height={24} /></span>
	<span class="copy"
		><span class="destination"
			>{#if show_source}{source_config[departure.route.data.source].name} ·
			{/if}{destination}</span
		>
		{#if show_stop || show_distance}<span class="place"
				>{#if show_stop}<span class="stop-name">{boarding.stop.name}</span>{/if}{#if show_distance}
					{#if show_stop}<span class="separator" aria-hidden="true">·</span>{/if}<span
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
