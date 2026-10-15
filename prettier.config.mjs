import { createRequire } from "node:module";

const require = createRequire(new URL("./frontend/package.json", import.meta.url));

export default {
  printWidth: 100,
  overrides: [
    {
      files: "frontend/**",
      options: {
        useTabs: true,
        singleQuote: true,
        trailingComma: "none",
        plugins: [
          "@ianvs/prettier-plugin-sort-imports",
          "prettier-plugin-svelte",
          "prettier-plugin-tailwindcss",
        ].map((plugin) => require.resolve(plugin)),
        tailwindStylesheet: "./frontend/src/app.css",
        importOrder: [
          "^svelte$",
          "",
          "^svelte/(.*)$",
          "",
          "^@sveltejs/(.*)$",
          "",
          "^\\$app/(.*)$",
          "",
          "^#lib/(.*)$",
          "",
          "<BUILTIN_MODULES>",
          "<THIRD_PARTY_MODULES>",
          "",
          "^[./]",
        ],
        importOrderTypeScriptVersion: "6.0.3",
      },
    },
    {
      files: "frontend/**/*.svelte",
      options: { parser: "svelte" },
    },
  ],
};
