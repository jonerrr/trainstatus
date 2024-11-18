<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { page } from '$app/stores';
	import { pushState } from '$app/navigation';
	import { onMount, tick } from 'svelte';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import { alerts } from '$lib/alerts.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';

	let { children } = $props();

	let last_update = $state<Date>(new Date());
	let last_st_update = $state<Date>(new Date());
	// let last_monitored_routes = $state<string>('');
	let offline = $state(false);

	onMount(async () => {
		const id =
			// stop
			$page.url.searchParams.get('s') ||
			// route
			$page.url.searchParams.get('r') ||
			// trip
			$page.url.searchParams.get('t');
		// console.log(id);
		if (id) {
			// check what type of id it is
			if (id in $page.data.routes) {
				await tick();
				pushState('', {
					modal: 'route',
					data: $page.data.routes[id]
				});
			} else if (id in $page.data.stops) {
				await tick();
				pushState('', {
					modal: 'stop',
					data: $page.data.stops[parseInt(id)]
				});
			} else if (trips.trips.has(id)) {
				await tick();
				pushState('', {
					modal: 'trip',
					data: trips.trips.get(id)
				});
			} else {
				console.error('Invalid ID', id);
				alert('Invalid ID');
			}
		}
	});

	let monitor_delay: number;

	$effect(() => {
		clearTimeout(monitor_delay);

		if (monitored_bus_routes.size > 30) {
			// remove until there are 30 left
			const to_remove = Array.from(monitored_bus_routes).slice(0, -30);
			// console.log('removing', to_remove);
			to_remove.forEach((r) => monitored_bus_routes.delete(r));
		}

		monitor_delay = setTimeout(() => {
			// console.log('updating stop times');
			stop_times.update(fetch, [...monitored_bus_routes]).then((o) => {
				console.log('updated');
				last_st_update = new Date();
				offline = o;
			});
		}, 50);
	});

	$effect(() => {
		const interval = setInterval(() => {
			// TODO: update more often if offline
			// TODO: exponential backoff
			if (new Date().getTime() - last_update.getTime() > 1000 * 10) {
				// console.log('Updating rt data');
				trips.update(fetch).then((o) => {
					offline = o;
				});
				alerts.update(fetch).then((o) => {
					offline = o;
				});

				last_update = new Date();
			}

			if (new Date().getTime() - last_st_update.getTime() > 1000 * 60) {
				// console.log('Updating stop times');
				stop_times.update(fetch, [...monitored_bus_routes]);
				last_st_update = new Date();
				// last_monitored_routes = [...monitored_bus_routes].join(',');
			}
		}, 200);

		return () => {
			clearInterval(interval);
		};
	});
</script>

<Header {offline} />
<main class="md:w-[60%] m-auto relative h-[calc(100dvh-7.5rem)]">
	<Modal />

	<!-- {#await data.initial_promise}
		<div class="text-neutral-50 text-4xl flex justify-center">
			<LoaderCircle size="4rem" class="animate-spin" />
		</div>
	{:then _} -->
	{@render children()}
	<!-- {/await} -->
</main>
<Navbar />

<style lang="postcss">
	:global(body) {
		@apply bg-neutral-950;
	}
</style>
