import { defineConfig, globalIgnores } from 'eslint/config';
import nextVitals from 'eslint-config-next/core-web-vitals';
import nextTs from 'eslint-config-next/typescript';

// `lib/generated/*Wasm/` holds `wasm-bindgen`'s own output, copied in
// byte-for-byte by the seven `abi:*-wasm` generators; each one's `--check`
// compares the committed bytes against a fresh `wasm-bindgen` run, so that
// byte-identity to upstream is exactly what makes the module trustworthy.
// Nothing in there has an author we can fix: editing a file turns its
// `abi:*:verify` red, and post-processing it in the generator would mean
// maintaining a permanent patch against third-party codegen. `wasm-bindgen`
// already stamps `/* eslint-disable */` on the `.d.ts` files it emits and just
// omits it on the `.js`, which is the whole finding. The SDK classifies the
// same artifacts the same way in `packages/dclutch-sdk/eslint.config.mjs` —
// one class per kind of file, in both trees. Every other generated module here
// is our own emitter's output and stays linted, so a lint finding in one is
// still a bug to fix in its generator.
const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  globalIgnores(['.next/**', 'out/**', 'build/**', 'next-env.d.ts', 'lib/generated/*Wasm/**']),
]);

export default eslintConfig;
