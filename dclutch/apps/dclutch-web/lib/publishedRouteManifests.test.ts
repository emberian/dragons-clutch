import { describe, expect, it } from 'vitest';

import { publishedDirectRouteManifestV1 } from '@/lib/publishedRouteManifests';

describe('published route manifests', () => {
  it('answers null for a market this build has no published route for', () => {
    expect(publishedDirectRouteManifestV1('57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP')).toBeNull();
  });
});
