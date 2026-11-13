import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const typesGenPath = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../src/lib/client/types.gen.ts'
);

/** @param {string} filePath */
function collectSnakeCaseProperties(filePath) {
	const text = readFileSync(filePath, 'utf8');
	const source = ts.createSourceFile(
		filePath,
		text,
		ts.ScriptTarget.Latest,
		true,
		ts.ScriptKind.TS
	);
	/** @type {Set<string>} */
	const names = new Set();
	const visit = (node) => {
		if (ts.isPropertySignature(node) || ts.isPropertyDeclaration(node)) {
			const { name } = node;
			if (ts.isIdentifier(name) && name.text.includes('_')) {
				names.add(name.text);
			}
		}
		if (
			ts.isTypeAliasDeclaration(node) &&
			node.name.text === 'Source' &&
			ts.isUnionTypeNode(node.type)
		) {
			for (const type of node.type.types) {
				if (ts.isLiteralTypeNode(type) && ts.isStringLiteral(type.literal)) {
					names.add(type.literal.text);
				}
			}
		}
		ts.forEachChild(node, visit);
	};
	visit(source);
	return [...names].sort();
}

/** API property names and Source dictionary keys with fixed wire spelling. */
export const openapiSnakeCaseProperties = collectSnakeCaseProperties(typesGenPath);
