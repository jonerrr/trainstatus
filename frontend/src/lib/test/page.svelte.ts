import { SvelteURL } from 'svelte/reactivity';

function emptyData(): App.PageData {
	return {
		selected_sources: [],
		stops: {},
		stops_by_id: {},
		routes: {},
		routes_by_id: {}
	};
}

export const page = $state<{ state: App.PageState; url: SvelteURL; data: App.PageData }>({
	state: { modal: null },
	url: new SvelteURL('http://localhost/'),
	data: emptyData()
});

export async function goto(url: string | URL, options?: { state?: App.PageState }) {
	page.url = new SvelteURL(url, page.url);
	if (options && 'state' in options && options.state !== undefined) {
		page.state = options.state;
	}
}

export function reset() {
	page.state = { modal: null };
	page.url = new SvelteURL('http://localhost/');
	page.data = emptyData();
}
