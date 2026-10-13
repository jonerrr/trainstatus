<script lang="ts">
	import '../../app.css';

	import { onDestroy } from 'svelte';

	import { chartFixture } from '../charts/fixtures';
	import Modal from '../Modal.svelte';
	import { setTransitData } from '../transit/store.svelte';
	import { createCurrentTime, setCurrentTime } from '../url_params.svelte';
	import { fixtureTransitData } from './transit-fixture';

	setCurrentTime(createCurrentTime());
	const transit = setTransitData(
		fixtureTransitData({
			catalogs: {
				njt_bus: {
					...chartFixture('njt_bus', 0).catalog,
					routes: ['First', 'Second', 'Third'].map((id) => ({
						...chartFixture('njt_bus', 0).catalog.routes[0],
						id,
						short_name: id
					}))
				}
			},
			services: {}
		})
	);
	onDestroy(() => transit.destroy());
</script>

<button data-background>Background</button>
<Modal />
