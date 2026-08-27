/**
 * Build-time feature flags.
 *
 * The Pages build is a static export, so a flag must be inlined at build time:
 * `NEXT_PUBLIC_*` variables are the one mechanism that survives it. Routes
 * always exist; a flag only decides whether the landing page points at them.
 */

/** Show the smoke-exchange story on the landing surface. */
export function smokeStoryEnabledV1(): boolean {
  return process.env.NEXT_PUBLIC_DCLUTCH_SMOKE === '1';
}
