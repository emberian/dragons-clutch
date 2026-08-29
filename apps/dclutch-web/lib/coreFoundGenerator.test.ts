import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

describe('Core Found ABI generator', () => {
  it('parses the Rust-owned Found frame and reproduces the checked-in bytes', () => {
    const app = fileURLToPath(new URL('..', import.meta.url));
    expect(() => execFileSync(process.execPath, ['scripts/generate-core-found.mjs', '--check'], {
      cwd: app,
      encoding: 'utf8',
      stdio: 'pipe',
    })).not.toThrow();
  });
});
