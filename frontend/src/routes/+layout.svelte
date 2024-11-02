<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { page } from '$app/stores';
	import { pushState } from '$app/navigation';
	import { onMount, tick } from 'svelte';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_routes } from '$lib/stop_times.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';
	import Modal from '$lib/Modal.svelte';
	import { alerts } from '$lib/alerts.svelte';
	import { trip_pins_rune } from '$lib/util.svelte';
	// import { LoaderCircle } from 'lucide-svelte';

	let { children } = $props();

	let last_update = $state<Date>(new Date());
	let last_st_update = $state<Date>(new Date());
	let last_monitored_routes = $state<string>('');
	let offline = $state(false);

	// $inspect(monitored_routes);

	onMount(async () => {
		// TODO: error handling

		const id = $page.url.searchParams.get('d');
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
				console.error('Invalid id', id);
			}
		}

		// page.subscribe((val) => {
		// 	console.log(val.state);
		// });
	});

	const monitored_routes_arr = $derived(
		Array.from(new Set(Array.from(monitored_routes.values()).flatMap((r) => r.map((id) => id))))
	);

	// $inspect(monitored_routes_arr);

	$effect(() => {
		if (monitored_routes_arr.join(',') === last_monitored_routes) return;
		// console.log('updating stop times', monitored_routes_arr);
		stop_times.update(fetch, monitored_routes_arr).then((o) => {
			// last_st_update = new Date();
			offline = o;
		});
		last_monitored_routes = monitored_routes_arr.join(',');
		last_st_update = new Date();
	});

	$effect(() => {
		const interval = setInterval(() => {
			// TODO: update more often if offline
			// TODO: exponential backoff
			if (new Date().getTime() - last_update.getTime() > 1000 * 10) {
				// console.log('Updating rt data');
				trips.update(fetch).then((o) => {
					// last_st_update = new Date();
					offline = o;
				});
				alerts.update(fetch).then((o) => {
					// last_st_update = new Date();
					offline = o;
				});

				last_update = new Date();
			}

			if (new Date().getTime() - last_st_update.getTime() > 1000 * 60) {
				// console.log('Updating stop times');
				stop_times.update(fetch, monitored_routes_arr);
				last_st_update = new Date();
				last_monitored_routes = monitored_routes_arr.join(',');
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
