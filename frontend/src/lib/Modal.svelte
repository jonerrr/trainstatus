<script lang="ts">
	import { flushSync, untrack } from 'svelte';

	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';

	import { page } from '$app/state';

	import Pin from '#lib/Pin.svelte';
	import RouteModal from '#lib/Route/Modal.svelte';
	import StopModal from '#lib/Stop/Modal.svelte';
	import TripModal from '#lib/Trip/Modal.svelte';
	import type { Source } from '#lib/client/index.js';
	import { type Pins, route_pins, stop_pins, trip_pins } from '#lib/pins.svelte.js';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { close_modal } from '#lib/url_params.svelte.js';

	import {
		AlarmClock,
		CircleX,
		ClipboardCheck,
		RotateCcwClock,
		Share,
		Timer
	} from '@lucide/svelte';

	// TODO: make implement some sort of focus trap and restore using attachments (actually, i think the dialog element does this natively?)

	// by reassigning the page.state locally, we can ensure the dialog transitions run before the DOM updates.
	// Otherwise, the sliding animation looks like it runs twice.
	let current_page_state = $state(page.state);

	$effect(() => {
		// $inspect.trace('modal state transition effect');
		const next_state = page.state;
		if (untrack(() => next_state.modal !== current_page_state.modal)) {
			// Compare the new index against the current index to figure out slide direction
			const next_index = next_state.index ?? 0;
			const local_index = untrack(() => current_page_state?.index ?? 0);

			const is_forward = next_index > local_index;
			document.documentElement.dataset.modalDirection = is_forward ? 'forward' : 'backward';

			// experimental.async makes tick() wait for a frame, which a view transition
			// defers until this callback finishes. Flush on a microtask so we aren't
			// calling flushSync from inside the effect.
			if (document.startViewTransition) {
				document.startViewTransition(
					() =>
						new Promise<void>((resolve) => {
							queueMicrotask(() => {
								// Runs the attachment effect below before the new snapshot.
								flushSync(() => {
									current_page_state = next_state;
								});
								resolve();
							});
						})
				);
			} else {
				current_page_state = next_state;
			}
		}
	});

	const modal: Attachment<HTMLDialogElement> = (node) => {
		document.body.style.overflow = 'hidden';

		// showModal() is what puts the dialog in the top layer. The open attribute does not.
		// flushSync in the view transition runs this before the new snapshot is taken.
		$effect(() => {
			const has_modal = !!current_page_state.modal;
			if (has_modal && !node.open) {
				node.showModal();
			} else if (!has_modal && node.open) {
				node.close();
			}
		});

		// watch for clicks outside the dialog to close it
		// function handle_click(event: MouseEvent) {
		// 	if (event.target === node) {
		// 		close_modal();
		// 	}
		// }

		// Add keyboard handler for Escape key
		function handle_keydown(event: KeyboardEvent) {
			if (event.key === 'Escape') {
				event.preventDefault();
				close_modal();
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
				close_modal();
			}
		}

		const listeners_to_remove: Array<() => void> = [];

		// listeners_to_remove.push(on(node, 'click', handle_click));
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
				close_modal();
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
					<RotateCcwClock size="2rem" />
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
	{#if current_page_state.modal?.type === 'stop'}
		<StopModal stop={current_page_state.modal} {show_previous} time_format={time_format.current} />

		{@render actions(
			true,
			's',
			current_page_state.modal.id,
			`Arrivals at ${current_page_state.modal.name}`,
			current_page_state.modal.data.source,
			stop_pins
		)}
	{:else if current_page_state.modal?.type === 'route'}
		<RouteModal route={current_page_state.modal} time_format={time_format.current} />

		{@render actions(
			false,
			'r',
			current_page_state.modal.id,
			`Alerts for ${current_page_state.modal.short_name}`,
			current_page_state.modal.data.source,
			route_pins
		)}
	{:else if current_page_state.modal?.type === 'trip'}
		<TripModal trip={current_page_state.modal} {show_previous} time_format={time_format.current} />

		{@render actions(
			true,
			't',
			current_page_state.modal.id,
			`${current_page_state.modal.route_id} Trip`,
			current_page_state.modal.data.source,
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
