<script lang="ts">
	import '../../app.css';

	import { onDestroy } from 'svelte';

	import Modal from '../Modal.svelte';
	import { setTransitData } from '../transit/store.svelte';
	import { createCurrentTime, setCurrentTime } from '../url_params.svelte';
	import { fixtureTransitData } from './transit-fixture';

	setCurrentTime(createCurrentTime());
	const transit = setTransitData(
		fixtureTransitData({
			catalogs: { njt_bus: { source: 'njt_bus', routes: [], stops: [] } },
			services: {}
		})
	);
	onDestroy(() => transit.destroy());
</script>

<button data-background>Background</button>
<Modal />
