import type { Source } from '#lib/client/index.js';

export type DataKind = 'trips' | 'stop_times' | 'positions' | 'alerts' | 'trajectories';
export interface ResourceQuery {
	source: Source;
	kind: DataKind;
	at: number | null;
	routes: readonly string[];
	active: boolean;
	url: string;
	key: string;
}
export function resourceQuery(
	source: Source,
	kind: DataKind,
	at: number | null = null,
	routes: readonly string[] = [],
	active = true
): ResourceQuery {
	const sorted = Object.freeze([...new Set(routes)].sort());
	const params = new URLSearchParams();
	if (at !== null) params.set('at', String(at));
	if (sorted.length) params.set('route_ids', sorted.join(','));
	const url = `/api/v1/${kind}/${source}${params.size ? `?${params}` : ''}`;
	return Object.freeze({ source, kind, at, routes: sorted, active, url, key: `${url}:${active}` });
}
/** Share status checks and cancellation across JSON and Arrow requests. */
export async function requestData<T>(
	url: string,
	decode: (response: Response) => Promise<T>,
	signal: AbortSignal,
	fetcher: typeof fetch = fetch
): Promise<T> {
	const response = await fetcher(url, { signal });
	if (!response.ok) throw new Error(`Data service returned ${response.status}`);
	return decode(response);
}
