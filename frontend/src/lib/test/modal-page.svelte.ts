import { SvelteURL } from 'svelte/reactivity';

export const page = $state<{ state: App.PageState; url: SvelteURL }>({
	state: { modal: null },
	url: new SvelteURL('http://localhost/')
});
