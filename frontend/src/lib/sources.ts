import type { Source } from '#lib/client/index.js';

export const sourceConfig = {
	mta_subway: { name: 'MTA Subway', wholeService: true, refreshMs: 30_000 },
	mta_bus: { name: 'MTA Bus', wholeService: false, refreshMs: 30_000 },
	njt_bus: { name: 'NJT Bus', wholeService: false, refreshMs: 30_000 }
} as const satisfies Record<Source, unknown>;

export const allSources = Object.keys(sourceConfig) as Source[];

export function isSource(value: string | null | undefined): value is Source {
	return value != null && Object.hasOwn(sourceConfig, value);
}
