import path from 'node:path';
import eslint from '@eslint/js';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, includeIgnoreFile } from 'eslint/config';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import { namingConventionRules, trainstatusPlugin } from './eslint-rules/index.mjs';

const generatedClient = 'src/lib/client/**';
const lintedSource = ['**/*.{js,mjs,ts,svelte}'];

export default defineConfig(
	includeIgnoreFile(path.resolve(import.meta.dirname, '../.gitignore')),
	eslint.configs.recommended,
	...tseslint.configs.recommended,
	...svelte.configs['flat/recommended'],
	prettier,
	...svelte.configs['flat/prettier'],
	{
		languageOptions: {
			globals: {
				...globals.browser,
				...globals.node
			}
		},
		rules: { 'no-undef': 'off' }
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: tseslint.parser
			}
		}
	},
	{
		files: lintedSource,
		ignores: [generatedClient],
		plugins: { trainstatus: trainstatusPlugin },
		rules: {
			'@typescript-eslint/naming-convention': ['error', ...namingConventionRules],
			'trainstatus/path-naming': 'error'
		}
	},
	{
		ignores: ['build/', '.svelte-kit/', 'dist/', generatedClient]
	}
);
