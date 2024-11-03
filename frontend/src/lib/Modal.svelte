<script lang="ts">
	import { CircleX, Share, ClipboardCheck, History, Timer, AlarmClock } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { pushState } from '$app/navigation';
	import { page } from '$app/stores';
	import {
		stop_pins_rune,
		trip_pins_rune,
		route_pins_rune,
		type PersistedRune,
		persisted_rune
	} from '$lib/util.svelte';
	import StopModal from '$lib/Stop/Modal.svelte';
	import TripModal from '$lib/Trip/Modal.svelte';
	import RouteModal from '$lib/Route/Modal.svelte';
	import Pin from './Pin.svelte';

	function close() {
		// enable_scroll();
		pushState('', { modal: null });
	}

	function manage_modal(node: HTMLDivElement) {
		document.body.style.overflow = 'hidden';

		// page.subscribe(({ state }) => {
		// 	if (state.modal) {
		// 		document.body.style.overflow = 'hidden';
		// 		// disable_scroll();
		// 	} else {
		// 		document.body.style.overflow = '';
		// 		// enable_scroll();
		// 	}
		// });

		// This differentiates between a drag and a click so mobile users don't accidentally close the dialog when swiping to go back
		// from here https://stackoverflow.com/a/59741870
		const delta = 6;
		let startX: number;
		let startY: number;

		function handle_mouse_down(event: MouseEvent) {
			if (event.target === node) {
				startX = event.pageX;
				startY = event.pageY;
			}
		}

		node.addEventListener('mousedown', handle_mouse_down);

		function handle_mouse_up(event: MouseEvent) {
			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);
			// console.log(event.target.id);

			if (diffX < delta && diffY < delta) {
				// Close the dialog
				close();
			}
		}

		node.addEventListener('mouseup', handle_mouse_up);

		return {
			destroy() {
				document.body.style.overflow = '';
				node.removeEventListener('mousedown', handle_mouse_down);
				node.removeEventListener('mouseup', handle_mouse_up);
			}
		};
	}

	// manage title changes
	onMount(() => {
		page.subscribe(({ state, route }) => {
			// console.log(route, state.modal);
			switch (state.modal) {
				case 'route':
					document.title = `Alerts for ${state.data.id}`;
					break;
				case 'stop':
					document.title = `Arrivals at ${state.data.name}`;
					break;
				case 'trip':
					document.title = `${state.data.route_id} Trip`;
					break;
				default:
					switch (route.id) {
						case '/stops':
							document.title = 'Stops';
							break;
						case '/alerts':
							document.title = 'Alerts';
							break;
						default:
							document.title = 'TrainStat.us';
							break;
					}
					break;
			}
		});
	});

	let copied = $state(false);
	// show stops/trips before current datetime
	let show_previous = $state(false);
	let time_format = persisted_rune<'countdown' | 'time'>('time_format', 'countdown');
</script>

{#snippet actions(
	history: boolean,
	id: string | number,
	title: string,
	pin_rune: PersistedRune<(string | number)[]>
)}
	<div class="flex gap-1 items-center justify-between px-1 h-16">
		<button
			onclick={() => {
				close();
			}}
			aria-label="Close modal"
			title="Close modal"
		>
			<CircleX size="2rem" />
		</button>

		<div class="flex gap-1 items-center text-xs">
			<!-- TODO: make history button work -->
			<!-- {#if history}
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
			{/if} -->

			<button
				class="flex flex-col items-center"
				aria-label="Change time formatting"
				title="Change time formatting"
				onclick={() => {
					time_format.value = time_format.value === 'countdown' ? 'time' : 'countdown';
				}}
			>
				{#if time_format.value === 'countdown'}
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
						const url = `${window.location.origin}?d=${id}`;

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
				<button class="appearance-none flex text-green-600" aria-label="Link copied to clipboard">
					<ClipboardCheck size="2rem" />
				</button>
			{/if}

			<Pin {id} {pin_rune} size="2rem" />
		</div>
	</div>
{/snippet}

<!-- close modal on escape key -->
<svelte:window onkeydown={($event) => $page.state.modal && $event.key == 'Escape' && close()} />

{#if $page.state.modal}
	<div
		use:manage_modal
		class="fixed top-0 left-0 flex flex-col justify-center items-center w-[100dvw] h-[100dvh] z-50 bg-black/50 bg-opacity-10 text-neutral-100"
	>
		<!-- transition:slide={{ duration: 150 }} -->
		<div
			role="dialog"
			aria-modal="true"
			class="snap-start bg-neutral-900 w-full md:w-[60%] rounded flex flex-col fixed bottom-0"
		>
			{#if $page.state.modal === 'stop'}
				<StopModal {show_previous} time_format={time_format.value} stop={$page.state.data} />

				{@render actions(
					true,
					$page.state.data.id,
					`Arrivals at ${$page.state.data.name}`,
					stop_pins_rune
				)}
			{:else if $page.state.modal === 'route'}
				<RouteModal route={$page.state.data} time_format={time_format.value} />

				{@render actions(
					true,
					$page.state.data.id,
					`Alerts for ${$page.state.data.id}`,
					route_pins_rune
				)}
			{:else if $page.state.modal === 'trip'}
				<TripModal trip={$page.state.data} {show_previous} time_format={time_format.value} />

				{@render actions(
					true,
					$page.state.data.id,
					`${$page.state.data.route_id} Trip`,
					trip_pins_rune
				)}
			{/if}
		</div>
	</div>
{/if}

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
