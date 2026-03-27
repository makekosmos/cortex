import eslint from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsparser from "@typescript-eslint/parser";
import svelte from "eslint-plugin-svelte";
import globals from "globals";

export default [
    eslint.configs.recommended,
    ...svelte.configs["flat/recommended"],
    {
        files: ["**/*.ts"],
        languageOptions: {
            parser: tsparser,
            parserOptions: {
                ecmaVersion: "latest",
                sourceType: "module",
            },
            globals: {
                ...globals.browser,
                ...globals.node,
            },
        },
        plugins: {
            "@typescript-eslint": tseslint,
        },
        rules: {
            ...tseslint.configs.recommended.rules,
            // TypeScript already knows global types like RequestInit.
            // `no-undef` can incorrectly flag type-only identifiers.
            "no-undef": "off",
            "@typescript-eslint/no-unused-vars": ["warn", { argsIgnorePattern: "^_" }],
        },
    },
    {
        files: ["**/*.svelte"],
        languageOptions: {
            parserOptions: {
                parser: tsparser,
            },
            globals: {
                ...globals.browser,
            },
        },
        rules: {
            // Svelte templates + TypeScript types confuse the base rules.
            // TypeScript/Svelte tooling already catches real issues here.
            "no-undef": "off",
            "no-unused-vars": "off",
        },
    },
    {
        ignores: ["node_modules/", "dist/", "src-tauri/"],
    },
];
