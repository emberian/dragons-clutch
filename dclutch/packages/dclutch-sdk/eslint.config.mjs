import tseslint from 'typescript-eslint';

// The web app lints with eslint-config-next, which assumes React and Next.
// This package has neither, by contract — so it carries the plain
// typescript-eslint recommended set instead. Generated modules are linted
// too: they are committed source, and a generator that emits a lint finding
// should be fixed at the generator.
export default tseslint.config(
  ...tseslint.configs.recommended,
  {
    ignores: ['node_modules/**'],
  },
  {
    rules: {
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  },
);
