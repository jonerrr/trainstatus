const ROOT = '/frontend/';
const KEBAB = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const PASCAL = /^[A-Z][a-zA-Z0-9]*$/;

/** SvelteKit route entries only (no invalid cross-products like +server.svelte). */
const KIT_ROUTE_BASENAMES = new Set([
	'+page.svelte',
	'+page.ts',
	'+page.js',
	'+page.server.ts',
	'+page.server.js',
	'+page.test.ts',
	'+page.server.test.ts',
	'+layout.svelte',
	'+layout.ts',
	'+layout.js',
	'+layout.server.ts',
	'+layout.server.js',
	'+layout.test.ts',
	'+layout.server.test.ts',
	'+error.svelte',
	'+error.ts',
	'+error.js',
	'+error.test.ts',
	'+server.ts',
	'+server.js',
	'+server.test.ts'
]);

/** @param {string} absolute */
function rel(absolute) {
	const p = absolute.replaceAll('\\', '/');
	const i = p.lastIndexOf(ROOT);
	return i === -1 ? null : p.slice(i + ROOT.length);
}

/** @param {string} seg */
function kitDirSegment(seg) {
	return (
		KEBAB.test(seg) ||
		/^\([^()/]+\)$/.test(seg) ||
		/^@[\w[\].-]+$/.test(seg) ||
		/^\[+[^[\]]+\]+$/.test(seg)
	);
}

/** @param {string} name */
function testStem(name) {
	if (name.endsWith('.svelte.test.ts')) return name.slice(0, -'.svelte.test.ts'.length);
	if (name.endsWith('.e2e.ts')) return name.slice(0, -'.e2e.ts'.length);
	if (name.endsWith('.test.ts')) return name.slice(0, -'.test.ts'.length);
	return null;
}

/**
 * @param {string} path
 * @returns {string[]}
 */
function violations(path) {
	const parts = path.split('/');
	const file = parts.at(-1) ?? '';
	const parent = parts.slice(0, -1).join('/');
	const inRoutesTree = path === 'src/routes' || path.startsWith('src/routes/');
	const atSrcRoot = parent === 'src';
	/** @type {string[]} */
	const errors = [];
	let underRoutes = false;

	for (let i = 0; i < parts.length - 1; i++) {
		const seg = parts[i];
		if (seg === 'routes' && parts[i - 1] === 'src') underRoutes = true;
		const ok = underRoutes ? kitDirSegment(seg) : KEBAB.test(seg);
		if (!ok) {
			errors.push(
				PASCAL.test(seg)
					? `directory "${seg}" must be kebab-case (e.g. Route → route)`
					: `directory "${seg}" must be kebab-case or a valid SvelteKit route segment`
			);
		}
	}

	// hooks.ts is universal; server/client variants optional.
	if (atSrcRoot && /^hooks(\.(server|client))?(\.test)?\.ts$/.test(file)) return errors;
	if (file === 'ambient.d.ts' || file === 'app.d.ts' || file === 'env.ts') return errors;
	if (inRoutesTree && KIT_ROUTE_BASENAMES.has(file)) return errors;
	if (
		/^(eslint|stylelint|svelte|vite|vitest|playwright|openapi-ts)\.config\.(js|mjs|ts)$/.test(file)
	) {
		return errors;
	}

	if (/\.svelte$/.test(file)) {
		const base = file.slice(0, -7);
		if (!PASCAL.test(base)) errors.push(`Svelte component "${file}" must use PascalCase`);
		return errors;
	}

	if (!/\.(ts|js|mjs)$/.test(file)) return errors;

	const stem = testStem(file);
	if (stem != null) {
		const svelteTest = file.endsWith('.svelte.test.ts');
		const ok = svelteTest ? PASCAL.test(stem) || KEBAB.test(stem) : KEBAB.test(stem);
		if (!ok) errors.push(`test module "${stem}" must be kebab-case or Component.svelte.test.ts`);
		return errors;
	}

	const base = file.replace(/\.(svelte\.ts|mjs|ts|js)$/, '');
	if (PASCAL.test(base))
		errors.push(`module "${base}" must be kebab-case (PascalCase is for .svelte only)`);
	else if (!KEBAB.test(base)) errors.push(`module "${base}" must be kebab-case`);
	return errors;
}

/** @type {import('eslint').Rule.RuleModule} */
export const pathNamingRule = {
	meta: {
		type: 'problem',
		docs: {
			description: 'kebab-case modules/dirs; PascalCase components; explicit SvelteKit names'
		},
		schema: [],
		messages: { invalidPath: '{{message}} ({{path}})' }
	},
	create(context) {
		const path = rel(context.filename ?? context.physicalFilename);
		if (!path || path.startsWith('src/lib/client/') || path.startsWith('src/lib/assets/')) {
			return {};
		}
		const issues = violations(path);
		if (!issues.length) return {};
		return {
			Program(node) {
				for (const message of issues) {
					context.report({ node, messageId: 'invalidPath', data: { message, path } });
				}
			}
		};
	}
};
