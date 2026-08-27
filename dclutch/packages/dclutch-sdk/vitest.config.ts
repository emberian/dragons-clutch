import { defineConfig } from 'vitest/config';

// The SDK is connection-agnostic and browser-free by contract, so the suite
// runs in a plain node environment; there is nothing to jsdom. Everything in
// lib/ imports relatively, so no alias is required either — an import that
// needs one is an import that would break a consumer.
export default defineConfig({
  test: {
    environment: 'node',
  },
});
