import type { CompassDirection } from '$lib/client';

interface CompassDirectionMetadata {
	transfer_order: number;
	rotation: number | null;
}

/**
 * Runtime metadata for every generated CompassDirection value.
 *
 * Key order preserves the direction filter's display order. The Record check
 * makes a generated type change fail type-checking until this metadata is
 * updated as well.
 */
export const COMPASS_DIRECTIONS = {
	sw: { transfer_order: 5, rotation: 225 },
	s: { transfer_order: 4, rotation: 180 },
	se: { transfer_order: 3, rotation: 135 },
	e: { transfer_order: 2, rotation: 90 },
	w: { transfer_order: 6, rotation: 270 },
	ne: { transfer_order: 1, rotation: 45 },
	nw: { transfer_order: 7, rotation: 315 },
	n: { transfer_order: 0, rotation: 0 },
	unknown: { transfer_order: 8, rotation: null }
} as const satisfies Record<CompassDirection, CompassDirectionMetadata>;

export const COMPASS_DIRECTION_OPTIONS = Object.keys(COMPASS_DIRECTIONS) as CompassDirection[];
