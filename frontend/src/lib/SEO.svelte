<script lang="ts">
	import { page } from '$app/state';

	const url_mappings = {
		'/': 'Home',
		'/stops': 'Stops',
		'/routes': 'Routes',
		'/alerts': 'Alerts',
		'/charts': 'Charts',
		'/map': 'Map',
		'/settings': 'Settings'
	} as const;

	const title = $derived.by(() => {
		switch (page.state.modal?.type) {
			case 'stop':
				return `Stop: ${page.state.modal.name}`;
			case 'route':
				return `Route: ${page.state.modal.short_name}`;
			case 'trip':
				return `${page.state.modal.route_id} Trip`;
			// TODO: settings
		}

		const title =
			page.url.pathname in url_mappings
				? url_mappings[page.url.pathname as keyof typeof url_mappings]
				: '???'; // shouldn't happen since we don't have any other routes, but just in case

		return `${title} | Train Status`;
	});
	// TODO: improve descriptions
	// TODO: include details about current page in meta tags
</script>

<svelte:head>
	<title>{title}</title>

	<meta
		name="description"
		content="The best website to view MTA subway (and bus) times and alerts."
	/>

	<!-- OGP Tags -->
	<meta property="og:title" content={title} />
	<meta property="og:type" content="website" />
	<meta
		property="og:description"
		content="The best website to view MTA subway (and bus) times and alerts."
	/>

	<!-- Twitter Meta Tags -->
	<meta name="twitter:card" content="summary_large_image" />
	<meta property="twitter:domain" content="Train Status" />
	<meta property="twitter:url" content="https://trainstat.us" />
	<meta name="twitter:title" content={title} />
	<meta
		name="twitter:description"
		content="The best website to view MTA subway (and bus) times and alerts."
	/>
</svelte:head>
