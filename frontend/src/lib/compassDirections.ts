import type { CompassDirection } from '#lib/client/index.js';

interface CompassDirectionMetadata {
	rotation: number | null;
}

/**
 * Runtime metadata for every generated CompassDirection value.
 *
 * Key order preserves the compass filter's display order. The Record check
 * makes a generated type change fail type-checking until this metadata is
 * updated as well.
 */
export const COMPASS_DIRECTIONS = {
	sw: { rotation: 225 },
	s: { rotation: 180 },
	se: { rotation: 135 },
	e: { rotation: 90 },
	w: { rotation: 270 },
	ne: { rotation: 45 },
	nw: { rotation: 315 },
	n: { rotation: 0 },
	unknown: { rotation: null }
} as const satisfies Record<CompassDirection, CompassDirectionMetadata>;

export const COMPASS_DIRECTION_OPTIONS = Object.keys(COMPASS_DIRECTIONS) as CompassDirection[];
