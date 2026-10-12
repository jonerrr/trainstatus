<script lang="ts">
	import { prefersReducedMotion } from 'svelte/motion';
	import { slide } from 'svelte/transition';

	import { getTransitData } from '#lib/transit/store.svelte.js';

	import { CircleAlert, WifiOff } from '@lucide/svelte';

	const data = getTransitData();
	const status = data.status;

	const state = $derived(
		status.offline ? 'offline' : status.failing ? 'failing' : status.stale ? 'stale' : null
	);
	const label = $derived(
		state === 'offline'
			? 'Offline'
			: state === 'failing'
				? 'Updates unavailable'
				: 'Updates delayed'
	);
	const age = $derived.by(() => {
		if (status.last_fetched === null) return '';
		const minutes = Math.floor((Date.now() - status.last_fetched) / 60_000);
		return minutes < 1 ? 'last fetched just now' : `last fetched ${minutes} min ago`;
	});
</script>

<span class="sr-only" role="status">{state ? `${label}${age ? `, ${age}` : ''}` : ''}</span>
{#if state}
	<div
		class="overflow-hidden"
		transition:slide={{ duration: prefersReducedMotion.current ? 0 : 200 }}
	>
		<button
			type="button"
			class="update-control flex h-5 w-full items-center justify-center gap-2 bg-amber-950 text-xs text-amber-200 md:h-auto md:min-h-16 md:flex-col md:gap-1 md:py-2"
			aria-label={`${label}. Retry updates`}
			disabled={state === 'offline'}
			onclick={() => data.retry()}
		>
			{#if state === 'offline'}
				<WifiOff size={14} />
			{:else}
				<CircleAlert size={14} />
			{/if}
			<span>{label}</span>
			{#if age}<span class="text-amber-300/80">{age}</span>{/if}
			{#if state !== 'offline'}<span class="underline">Retry</span>{/if}
		</button>
	</div>
{/if}
