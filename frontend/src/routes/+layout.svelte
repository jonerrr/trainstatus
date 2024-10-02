<script lang="ts">
	import '../app.css';
	import '@fontsource/inter';
	import { trips } from '$lib/trips.svelte';
	import { stop_times, monitored_routes } from '$lib/stop_times.svelte';
	import Navbar from '$lib/Navbar.svelte';
	import Header from '$lib/Header.svelte';

	let { children } = $props();

	let last_update = $state<Date>();
	let last_monitored_routes = $state<string>('');

	// $inspect(monitored_routes);

	$effect(() => {
		if (monitored_routes.sort().toString() !== last_monitored_routes) {
			stop_times.update(monitored_routes);
			last_monitored_routes = monitored_routes.sort().toString();
			last_update = new Date();
		}

		const interval = setInterval(() => {
			// TODO: update more often if offline
			// TODO: exponential backoff
			if (!last_update || new Date().getTime() - last_update.getTime() > 1000 * 10) {
				console.log('Updating');
				trips.update();
				stop_times.update(monitored_routes);
				last_update = new Date();
			}
		}, 200);

		return () => {
			clearInterval(interval);
		};
	});
</script>

<Header />
<main class="md:w-[60%] m-auto">
	{@render children()}
</main>
<Navbar />

<style lang="postcss">
	:global(body) {
		@apply bg-neutral-950;
	}
</style>
