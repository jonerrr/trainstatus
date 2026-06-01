export const VEHICLE_ICON_MAPPING = {
	rail_head: {
		x: 0,
		y: 0,
		width: 36,
		height: 180,
		anchorX: 18,
		anchorY: 90,
		mask: true
	},
	rail_car: {
		x: 40,
		y: 0,
		width: 36,
		height: 180,
		anchorX: 18,
		anchorY: 90,
		mask: true
	},
	bus: {
		x: 80,
		y: 0,
		width: 48,
		height: 120,
		anchorX: 24,
		anchorY: 60,
		mask: true
	}
} as const;

const VEHICLE_ICON_ATLAS_SVG = `
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="180" viewBox="0 0 128 180">
	<rect width="128" height="180" fill="transparent" />
	<path d="M18 8 L32 28 L32 172 L4 172 L4 28 Z" fill="white" rx="6" />
	<rect x="44" y="8" width="28" height="164" rx="6" fill="white" />
	<rect x="84" y="18" width="40" height="144" rx="8" fill="white" />
	<rect x="92" y="28" width="24" height="24" rx="4" fill="black" opacity="0.15" />
	<rect x="92" y="58" width="24" height="76" rx="4" fill="black" opacity="0.1" />
</svg>
`.trim();

export const VEHICLE_ICON_ATLAS = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(
	VEHICLE_ICON_ATLAS_SVG
)}`;
