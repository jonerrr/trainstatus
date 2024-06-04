<script lang="ts">
	import { page } from '$app/stores';

	export let href = '/';
	export let label = 'Home';
</script>

<!-- adding pb so navbar on ios isn't in the way -->
<a
	aria-label={label}
	{href}
	class={`block w-full h-full m-auto pb-2 ${$page.url.pathname === href ? 'text-fuchsia-700' : 'text-indigo-600'}`}
>
	<slot />
</a>
