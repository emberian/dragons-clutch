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

/** The repository, which is always a truthful destination for a document. */
const REPOSITORY_V1 = 'https://github.com/emberian/dragons-clutch';

/**
 * Where one document lives *for this build*.
 *
 * The Pages artifact renders `docs/` to HTML and mounts it beside the app
 * (`NEXT_PUBLIC_DCLUTCH_DOCS_BASE=/docs`), so there the app links to the page a
 * reader can actually read. Every other build — `vinext dev`, the Cloudflare
 * worker — ships the app alone, and linking to a rendered page that is not
 * deployed would be a promise the build cannot keep; those link to the Markdown
 * source in the repository instead.
 *
 * @param rendered site-relative HTML path under the docs base, e.g. `guides/README.html`
 * @param source repo-relative Markdown path, e.g. `docs/guides/README.md`
 */
export function docsHrefV1(rendered: string, source: string): string {
  const base = process.env.NEXT_PUBLIC_DCLUTCH_DOCS_BASE;
  return base
    ? `${base.replace(/\/+$/, '')}/${rendered}`
    : `${REPOSITORY_V1}/blob/main/dclutch/${source}`;
}

/** The repository itself. */
export function repositoryHrefV1(): string {
  return REPOSITORY_V1;
}
