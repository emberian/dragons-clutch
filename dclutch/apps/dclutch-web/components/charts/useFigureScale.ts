'use client';

import { useEffect, useRef, useState } from 'react';

/**
 * Label sizes for the figures, in REAL CSS PIXELS.
 *
 * Every chart here is drawn on a fixed unit grid and then stretched to whatever
 * width its slot happens to give it. That is right for the marks — a bar should
 * use the width it is given — and wrong for the text, because SVG text is
 * scaled by the same viewport transform as everything else. A `fontSize={8}`
 * label is 8 units, not 8 pixels, and the units are worth whatever the slot
 * makes them worth.
 *
 * Measured 2026-08-31 across the public routes, the same component rendered its
 * axis labels at:
 *
 *     /pulse        panel 1326px wide, viewBox 1000  →  scale 1.33  →  10.6px
 *     /markets      card   605px wide, viewBox 1000  →  scale 0.60  →   4.8px
 *     /population   cell   397px wide, viewBox 1000  →  scale 0.40  →   3.2px
 *
 * Three point two pixels. Nobody reads that, and nothing in the drawing code
 * said anything was wrong, because 8 is 8 wherever you write it.
 *
 * So a figure's label size is not a constant in the drawing. It is a constant
 * in pixels, converted back into user units by the ratio the browser is
 * actually painting at.
 */
export const FIGURE_LABEL_PX = 11;
export const FIGURE_AXIS_PX = 10;

/**
 * Measures what one user unit of a figure is currently worth in CSS pixels.
 *
 * Attach `figureRef` to the `<svg>` and size text with `units(FIGURE_LABEL_PX)`.
 *
 * Before the first measurement — server rendering, and the first paint of a
 * static export — the ratio is 1, so a label falls back to its size in units,
 * which is the behaviour these charts already had. It corrects on hydration and
 * on every resize after that.
 */
export function useFigureScale(viewBoxWidth: number) {
  const figureRef = useRef<SVGSVGElement | null>(null);
  const [pxPerUnit, setPxPerUnit] = useState(1);

  useEffect(() => {
    const svg = figureRef.current;
    if (!svg || viewBoxWidth <= 0) return;

    const measure = () => {
      const width = svg.getBoundingClientRect().width;
      if (width > 0) setPxPerUnit(width / viewBoxWidth);
    };
    measure();

    if (typeof ResizeObserver !== 'function') return;
    const observer = new ResizeObserver(measure);
    observer.observe(svg);
    return () => observer.disconnect();
  }, [viewBoxWidth]);

  return {
    figureRef,
    /** A size in CSS pixels, expressed in this figure's user units. */
    units: (px: number) => (pxPerUnit > 0 ? px / pxPerUnit : px),
  };
}
