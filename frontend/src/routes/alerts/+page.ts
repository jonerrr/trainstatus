import { fetch_alerts } from '$lib/api';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ fetch }) => {
	return {
		alerts: fetch_alerts(fetch)
	};
};
