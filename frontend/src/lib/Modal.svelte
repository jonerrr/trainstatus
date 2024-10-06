<script lang="ts">
	import { CircleX, Share, ClipboardCheck, CircleHelp, Dices, History } from 'lucide-svelte';
	import { pushState } from '$app/navigation';
	import { page } from '$app/stores';
	import {
		stop_pins_rune,
		trip_pins_rune,
		route_pins_rune,
		type PersistedRune
	} from '$lib/util.svelte';
	import type { Trip, TrainTripData, BusTripData } from './trips.svelte';
	import { type Route, type Stop } from './static';
	import StopModal from '$lib/Stop/Modal.svelte';
	import TripModal from '$lib/Trip/Modal.svelte';
	import Pin from './Pin.svelte';
	import { slide } from 'svelte/transition';

	let modal_el: HTMLDivElement;

	function close() {
		// enable_scroll();
		pushState('', { modal: null });
	}

	function manage_modal(node: HTMLDivElement) {
		page.subscribe(({ state }) => {
			if (state.modal) {
				document.body.style.overflow = 'hidden';
				// disable_scroll();
			} else {
				document.body.style.overflow = '';
				// enable_scroll();
			}
		});

		// This differentiates between a drag and a click so mobile users don't accidentally close the dialog when swiping to go back
		// from here https://stackoverflow.com/a/59741870
		const delta = 6;
		let startX: number;
		let startY: number;

		node.addEventListener('mousedown', function (event) {
			// Make sure the user is clicking outside of the dialog
			if (event.target === node) {
				startX = event.pageX;
				startY = event.pageY;
			}
		});

		node.addEventListener('mouseup', function (event) {
			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);
			// console.log(event.target.id);

			if (diffX < delta && diffY < delta) {
				// Close the dialog
				close();
			}
		});
	}

	let copied = $state(false);
	// show  stops/trips before current datetime
	let show_previous = $state(false);

	$effect(() => {
		page.subscribe((val) => {
			console.log(val.state);
		});
	});

	// for sharing, trip will be a uuid, stop will be a number, and route will be a string
</script>

{#snippet actions(
	history: boolean,
	id: string | number,
	pin_rune: PersistedRune<(string | number)[]>
)}
	<div class="flex gap-1 items-center justify-between pt-2">
		<button
			onclick={() => {
				close();
			}}
			aria-label="Close modal"
			class="appearance-none h-8 w-8 flex"
		>
			<CircleX />
		</button>

		<div class="flex">
			{#if history}
				<button
					class="appearance-none h-8 w-8 flex"
					class:text-indigo-600={show_previous}
					aria-label="Show previous"
					onclick={() => {
						show_previous = !show_previous;
					}}
				>
					<History />
				</button>
			{/if}

			{#if !copied}
				<button
					class="appearance-none h-8 w-8 flex"
					aria-label="Share"
					onclick={() => {
						const url = `${window.location.origin}/?d=${id}`;

						// Only use share api if on mobile and supported
						if (!navigator.share || !/Mobi/i.test(window.navigator.userAgent)) {
							navigator.clipboard.writeText(url);
							copied = true;
							setTimeout(() => {
								copied = false;
							}, 800);
						} else {
							navigator.share({
								// title: document.title,
								url
							});
						}
					}}
				>
					<Share class="h-6 w-6" />
				</button>
			{:else}
				<button
					class="appearance-none flex h-8 w-8 text-green-600"
					aria-label="Link copied to clipboard"
				>
					<ClipboardCheck class="h-6 w-6" />
				</button>
			{/if}

			<Pin {id} {pin_rune} class="appearance-none h-8 w-8 flex" />
		</div>
	</div>
{/snippet}

<!-- close modal on escape key -->
<svelte:window onkeydown={($event) => $page.state.modal && $event.key === 'Escape' && close()} />

{#if $page.state.modal}
	<div
		use:manage_modal
		class="fixed top-0 left-0 flex flex-col justify-center items-center w-[100dvw] h-[100dvh] z-50 bg-black/50 bg-opacity-10 text-neutral-100"
	>
		<div
			transition:slide={{ duration: 300 }}
			role="dialog"
			aria-modal="true"
			class="bg-neutral-900 w-full p-1 rounded flex flex-col fixed bottom-0"
		>
			{#if $page.state.modal === 'stop'}
				<!-- {@const stop = $page.state.data as Stop<'train' | 'bus'>} -->

				<StopModal bind:show_previous bind:stop={$page.state.data} />

				{@render actions(true, $page.state.data.id, stop_pins_rune)}
			{:else if $page.state.modal === 'route'}
				{@const route = $page.state.data as Route}

				{@render actions(true, route.id, route_pins_rune)}
			{:else if $page.state.modal === 'trip'}
				{@const trip = $page.state.data as Trip<TrainTripData | BusTripData>}

				<TripModal {trip} bind:show_previous />

				{@render actions(true, trip.id, trip_pins_rune)}
			{/if}
		</div>
	</div>
{/if}
