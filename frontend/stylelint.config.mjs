/** @type {import('stylelint').Config} */
export default {
	extends: ['stylelint-config-recommended'],
	ignoreFiles: ['build/**', '.svelte-kit/**', 'src/lib/client/**'],
	overrides: [{ files: ['**/*.svelte'], customSyntax: 'postcss-html' }],
	rules: {
		// Tailwind's @apply has a different grammar from the draft CSS @apply rule.
		'at-rule-prelude-no-invalid': [true, { ignoreAtRules: ['media', 'apply'] }],
		'at-rule-no-unknown': [
			true,
			{
				ignoreAtRules: [
					'theme',
					'source',
					'utility',
					'variant',
					'custom-variant',
					'apply',
					'reference',
					'config',
					'plugin'
				]
			}
		],
		'function-no-unknown': [true, { ignoreFunctions: ['--alpha', '--spacing', 'theme'] }],
		'selector-pseudo-class-no-unknown': [true, { ignorePseudoClasses: ['global'] }]
	}
};
