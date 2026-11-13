import mtaBusIcon from '#lib/assets/mta_bus.webp?w=24;48;72&enhanced';
import mtaSubwayIcon from '#lib/assets/mta_subway.webp?w=24;48;72&enhanced';
import njtBusIcon from '#lib/assets/njt_bus.webp?w=24;48;72&enhanced';
import type { Source } from '#lib/client/index.js';

// Keep image imports separate so non-UI consumers of sources.ts don't load image-processing dependencies.
export const sourceIcons = {
	mta_bus: mtaBusIcon,
	mta_subway: mtaSubwayIcon,
	njt_bus: njtBusIcon
} as const satisfies Record<Source, unknown>;
