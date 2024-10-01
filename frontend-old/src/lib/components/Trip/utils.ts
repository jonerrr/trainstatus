import { get } from 'svelte/store';
import { data_at } from '$lib/stores';

export function get_stop_times<K, T extends { stop_id: K; direction: number; arrival: Date }>(
	stop_times: T[],
	stop_id: K,
	direction: number
) {
	const now = get(data_at) || new Date();

	const st = stop_times.filter(
		(st) => st.stop_id === stop_id && st.direction === direction && st.arrival > now
	);

	return st.map((st) => {
		const arrival = st.arrival.getTime();
		const eta = (arrival - now.getTime()) / 1000 / 60;

		st.eta = eta;
		return st;
	});
}
