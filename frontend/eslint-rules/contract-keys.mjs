/**
 * External keys not described as properties in the generated API client.
 * These are property exceptions only; local bindings still use camelCase.
 */
export const frontendContractSnakeProperties = [
	'API_ORIGIN', // Playwright child-process environment.
	'rail_head', // Backend icon_key values used as atlas dictionary keys.
	'rail_car',
	'Program', // ESLint AST visitor key.
	'shape_key', // Arrow column names are documented, not typed as JSON properties.
	'shape_distances',
	'shape_positions'
];
