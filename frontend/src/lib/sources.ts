import type { Source } from '@trainstatus/client';

export const source_info = {
	// TODO: increase refresh interval
	// TODO: add icons
	mta_bus: {
		name: 'MTA Bus',
		icon: 'TODO',
		refresh_interval: 5000,
		// this means that this source requires including specific routes in the query params
		// maybe find a better name for the param in the future
		monitor_routes: true
	},
	mta_subway: {
		name: 'MTA Subway',
		icon: 'TODO',
		refresh_interval: 5000,
		monitor_routes: false
	}
} as const;

export const default_sources: Source[] = ['mta_bus', 'mta_subway'] as const;
// TODO: allow changing sources
