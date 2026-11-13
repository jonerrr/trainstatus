/** @type {import('stylelint').Config} */
export default {
	extends: ['stylelint-config-recommended'],
	ignoreFiles: ['build/**', '.svelte-kit/**', 'src/lib/client/**'],
	overrides: [{ files: ['**/*.svelte'], customSyntax: 'postcss-html' }],
	rules: {
		'at-rule-no-unknown': [true, { ignoreAtRules: ['theme', 'plugin'] }],
		'selector-pseudo-class-no-unknown': [true, { ignorePseudoClasses: ['global'] }]
	}
};
