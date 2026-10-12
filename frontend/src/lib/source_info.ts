import mta_bus_icon from '#lib/assets/mta_bus.webp?w=24;48;72&enhanced';
import mta_subway_icon from '#lib/assets/mta_subway.webp?w=24;48;72&enhanced';
import njt_bus_icon from '#lib/assets/njt_bus.webp?w=24;48;72&enhanced';
import type { Source } from '#lib/client/index.js';

export const source_info = {
	mta_bus: { name: 'MTA Bus', icon: mta_bus_icon },
	mta_subway: { name: 'MTA Subway', icon: mta_subway_icon },
	njt_bus: { name: 'NJT Bus', icon: njt_bus_icon }
} as const satisfies Record<Source, unknown>;

export const all_sources = Object.keys(source_info) as Source[];
