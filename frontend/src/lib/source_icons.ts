import mta_bus_icon from '#lib/assets/mta_bus.webp?w=24;48;72&enhanced';
import mta_subway_icon from '#lib/assets/mta_subway.webp?w=24;48;72&enhanced';
import njt_bus_icon from '#lib/assets/njt_bus.webp?w=24;48;72&enhanced';
import type { Source } from '#lib/client/index.js';

// Keep image imports separate so non-UI consumers of sources.ts don't load image-processing dependencies.
export const source_icons = {
	mta_bus: mta_bus_icon,
	mta_subway: mta_subway_icon,
	njt_bus: njt_bus_icon
} as const satisfies Record<Source, unknown>;
