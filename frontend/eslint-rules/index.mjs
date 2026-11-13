import { pathNamingRule } from './path-naming.mjs';

/** @type {import('eslint').ESLint.Plugin} */
export const trainstatusPlugin = {
	rules: {
		'path-naming': pathNamingRule
	}
};

export { namingConventionRules } from './naming-convention.mjs';
