import type { Source } from '#lib/client/index.js';

export const source_config = {
	mta_subway: { name: 'MTA Subway', whole_service: true, refresh_ms: 30_000 },
	mta_bus: { name: 'MTA Bus', whole_service: false, refresh_ms: 30_000 },
	njt_bus: { name: 'NJT Bus', whole_service: false, refresh_ms: 30_000 }
} as const satisfies Record<Source, unknown>;

export const all_sources = Object.keys(source_config) as Source[];

export function is_source(value: string | null | undefined): value is Source {
	return value != null && Object.hasOwn(source_config, value);
}
