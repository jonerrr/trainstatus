<script lang="ts">
	import { page } from '$app/stores';

	export let href = '/';
</script>

<a {href} class={`m-auto ${$page.url.pathname === href ? 'text-fuchsia-700' : 'text-indigo-600'}`}>
	<slot />
</a>
