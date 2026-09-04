import tseslint from 'typescript-eslint';

// The web app lints with eslint-config-next, which assumes React and Next.
// This package has neither, by contract — so it carries the plain
// typescript-eslint recommended set instead. Generated modules are linted
// too: they are committed source, and a generator that emits a lint finding
// should be fixed at the generator.
//
// `lib/generated/*Wasm/` is the ONE exception, and it is not a suppression:
// those directories are not our generator's output, they are `wasm-bindgen`'s,
// copied in byte-for-byte. `scripts/generate-rational-open-wasm.mjs` writes
// each of the four files with `readFileSync` from the `wasm-bindgen` run and
// its `--check` mode `.equals()`-compares all four, so the committed bytes ARE
// upstream's bytes and that identity is the whole reason the module can be
// trusted. There is therefore no author here to fix: editing the file turns
// `abi:rational-open-wasm:verify` red, and post-processing it in the generator
// would mean carrying a permanent patch against third-party codegen that a
// `wasm-bindgen` upgrade silently invalidates. `wasm-bindgen` agrees its output
// is not for consumer linters — it stamps `/* eslint-disable */` on the `.d.ts`
// files it emits, and merely omits it on the `.js`. So the rule is wrong for
// this directory, and the directory is named here rather than the finding being
// hidden at the site. Everything else under `lib/generated/` stays linted.
export default tseslint.config(
  ...tseslint.configs.recommended,
  {
    ignores: ['node_modules/**', 'lib/generated/*Wasm/**'],
  },
  {
    rules: {
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  },
);
