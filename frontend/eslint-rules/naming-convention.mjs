import { openapiSnakeCaseProperties } from './api-property-allowlist.mjs';
import { frontendContractSnakeProperties } from './contract-keys.mjs';

const externalSnakeCasePropertyPattern = `^(${[
	...new Set([...openapiSnakeCaseProperties, ...frontendContractSnakeProperties])
]
	.map((key) => key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
	.join('|')})$`;

export const namingConventionRules = [
	{ selector: 'import', format: ['camelCase', 'PascalCase'] },
	{ selector: 'typeLike', format: ['PascalCase'] },
	{ selector: 'enumMember', format: ['PascalCase', 'UPPER_CASE'] },
	{ selector: 'objectLiteralProperty', modifiers: ['requiresQuotes'], format: null },
	{
		selector: ['typeProperty', 'objectLiteralProperty', 'objectLiteralMethod'],
		format: null,
		filter: { regex: externalSnakeCasePropertyPattern, match: true }
	},
	{ selector: ['typeProperty', 'objectLiteralProperty'], format: ['camelCase'] },
	{ selector: 'typeProperty', modifiers: ['requiresQuotes'], format: null },
	{
		selector: 'variable',
		modifiers: ['const'],
		format: ['camelCase', 'PascalCase', 'UPPER_CASE'],
		leadingUnderscore: 'allow',
		trailingUnderscore: 'allow'
	},
	{
		selector: ['variable', 'parameter'],
		modifiers: ['destructured'],
		format: ['camelCase'],
		leadingUnderscore: 'allow'
	},
	{
		selector: 'variable',
		format: ['camelCase'],
		leadingUnderscore: 'allow',
		trailingUnderscore: 'allow'
	},
	{ selector: 'function', format: ['camelCase'], leadingUnderscore: 'allow' },
	{ selector: 'parameter', format: ['camelCase'], leadingUnderscore: 'allow' },
	{
		selector: ['classProperty', 'classMethod', 'accessor', 'method', 'property'],
		format: ['camelCase'],
		leadingUnderscore: 'allow'
	},
	{
		selector: 'default',
		format: ['camelCase'],
		leadingUnderscore: 'allow',
		trailingUnderscore: 'allow'
	}
];
