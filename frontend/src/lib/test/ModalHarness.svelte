<script lang="ts">
	import '../../app.css';

	import Modal from '../Modal.svelte';
	import { alert_context } from '../resources/alerts.svelte';
	import { createCurrentTime, setCurrentTime } from '../url_params.svelte';

	setCurrentTime(createCurrentTime());
	alert_context.set({});
</script>

<button data-background>Background</button>
<Modal />
