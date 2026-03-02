<script lang="ts">
	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';

	import { page } from '$app/state';

	import Pin from '$lib/Pin.svelte';
	import RouteModal from '$lib/Route/Modal.svelte';
	import StopModal from '$lib/Stop/Modal.svelte';
	import TripModal from '$lib/Trip/Modal.svelte';
	import { type Pins, route_pins, stop_pins, trip_pins } from '$lib/pins.svelte';
	import { LocalStorage } from '$lib/storage.svelte';
	import { close_modal } from '$lib/url_params.svelte';

	import { AlarmClock, CircleX, ClipboardCheck, History, Share, Timer } from '@lucide/svelte';
	import type { Source } from '@trainstatus/client';

	// TODO: make implement some sort of focus trap and restore using attachments

	function close() {
		close_modal();
	}
	// TODO: improve physics on desktop (or maybe just disable)
	const modal: Attachment<HTMLDialogElement> = (node) => {
		document.body.style.overflow = 'hidden';

		$effect(() => {
			// TODO: why does this run twice on close
			console.log(`modal update`, { state: page.state });
			if (page.state.modal) {
				node.showModal();
			} else {
				node.close();
			}
		});

		// // watch for clicks outside the dialog to close it
		function handle_click(event: MouseEvent) {
			if (event.target === node) {
				close();
			}
		}

		// Add keyboard handler for Escape key
		function handle_keydown(event: KeyboardEvent) {
			if (event.key === 'Escape') {
				event.preventDefault();
				close();
			}
		}

		// This differentiates between a drag and a click so mobile users don't accidentally close the dialog when swiping to go back
		// from here https://stackoverflow.com/a/59741870
		const delta = 6;
		let startX: number;
		let startY: number;

		function handle_mouse_down(event: MouseEvent) {
			if (event.target !== node) return;

			startX = event.pageX;
			startY = event.pageY;
		}

		function handle_mouse_up(event: MouseEvent) {
			// Only act if the mouse started and ended directly on the dialog node
			if (event.target !== node) return;

			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);

			if (diffX < delta && diffY < delta) {
				close();
			}
		}

		const listeners_to_remove: Array<() => void> = [];

		listeners_to_remove.push(on(node, 'click', handle_click));
		listeners_to_remove.push(on(node, 'mousedown', handle_mouse_down));
		listeners_to_remove.push(on(node, 'mouseup', handle_mouse_up));
		listeners_to_remove.push(on(document, 'keydown', handle_keydown));

		return () => {
			document.body.style.overflow = '';
			listeners_to_remove.forEach((off) => off());
		};
	};

	// const rotation = new Tween(0, {
	// 	duration: 300,
	// 	easing: cubicOut
	// });
	let copied = $state(false);
	// show stops/trips before current datetime TODO: maybe persist this preference in local storage
	let show_previous = $state(false);
	// e.g. 3m or 12:45.
	let time_format = new LocalStorage<'countdown' | 'time'>('time_format', 'countdown');
</script>

<!-- TODO: refactor actions now that we have sources -->
{#snippet actions(
	history: boolean,
	param_name: 'r' | 's' | 't',
	id: string,
	title: string,
	source: Source,
	pins: LocalStorage<Pins>
)}
	<div class="flex h-16 items-center justify-between gap-1 px-1">
		<button
			onclick={() => {
				close();
			}}
			aria-label="Close modal"
			title="Close modal"
		>
			<CircleX size="2rem" />
		</button>

		<div class="flex items-center gap-1 text-xs">
			{#if history}
				<button
					class:text-neutral-400={!show_previous}
					class:text-neutral-50={show_previous}
					aria-label="Show previous"
					onclick={() => {
						show_previous = !show_previous;
					}}
				>
					<History size="2rem" />
				</button>
			{/if}

			<!-- <style>
				@keyframes spin-forward {
					from {
						transform: rotate(0deg);
					}
					to {
						transform: rotate(360deg);
					}
				}

				@keyframes spin-backward {
					from {
						transform: rotate(0deg);
					}
					to {
						transform: rotate(-360deg);
					}
				}

				.spin-forward {
					animation: spin-forward 0.3s linear;
				}

				.spin-backward {
					animation: spin-backward 0.3s linear;
				}
			</style> -->

			<button
				class="flex flex-col items-center"
				aria-label="Change time formatting"
				title="Change time formatting"
				onclick={() => {
					time_format.current = time_format.current === 'countdown' ? 'time' : 'countdown';
				}}
			>
				{#if time_format.current === 'countdown'}
					<AlarmClock size="2rem" />
				{:else}
					<Timer size="2rem" />
				{/if}
				<!-- Time Format -->
			</button>

			{#if !copied}
				<button
					aria-label="Share"
					title="Share"
					onclick={() => {
						// URL already includes ?s/?r/?t and ?at params via shallow routing
						const url = window.location.href;

						// Only use share api if on mobile and supported
						if (!navigator.share || !/Mobi/i.test(window.navigator.userAgent)) {
							navigator.clipboard.writeText(url);
							copied = true;
							setTimeout(() => {
								copied = false;
							}, 800);
						} else {
							navigator.share({
								title,
								url
							});
						}
					}}
				>
					<Share size="2rem" />
				</button>
			{:else}
				<button class="flex appearance-none text-green-600" aria-label="Link copied to clipboard">
					<ClipboardCheck size="2rem" />
				</button>
			{/if}

			<Pin {id} {pins} {source} size="2rem" />
		</div>
	</div>
{/snippet}
<!-- fixed bottom-0 left-0 right-0 -->
<dialog
	{@attach modal}
	class="m-auto mb-0 flex max-h-[95dvh] w-full max-w-200 flex-col rounded-t-sm bg-neutral-900 text-white backdrop:bg-black/50 focus:ring-2 focus:ring-neutral-700 focus:outline-hidden"
>
	{#if page.state.modal?.type === 'stop'}
		<StopModal stop={page.state.modal} {show_previous} time_format={time_format.current} />

		{@render actions(
			true,
			's',
			page.state.modal.id,
			`Arrivals at ${page.state.modal.name}`,
			page.state.modal.data.source,
			stop_pins
		)}
	{:else if page.state.modal?.type === 'route'}
		<RouteModal route={page.state.modal} time_format={time_format.current} />

		{@render actions(
			false,
			'r',
			page.state.modal.id,
			`Alerts for ${page.state.modal.short_name}`,
			page.state.modal.data.source,
			route_pins
		)}
	{:else if page.state.modal?.type === 'trip'}
		<TripModal trip={page.state.modal} {show_previous} time_format={time_format.current} />

		{@render actions(
			true,
			't',
			page.state.modal.id,
			`${page.state.modal.route_id} Trip`,
			page.state.modal.data.source,
			trip_pins
		)}
	{/if}
</dialog>

<!-- <style>
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.spin {
		animation: spin 0.5s linear;
	}
</style> -->
<style>
	dialog[open] {
		view-transition-name: modal;
	}

	dialog::backdrop {
		background-color: rgb(0 0 0 / 50%);
	}
</style>
