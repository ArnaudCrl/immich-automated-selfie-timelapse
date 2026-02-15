<script>
  /**
   * Visual indicator for the max padding threshold.
   * Shows two nested squares: the full output frame (outer) and the
   * minimum real-pixel region (inner). The border zone between them
   * represents padding (duplicated edge pixels) and is tinted red.
   *
   * @param {number} maxPaddingPercent - Max allowed padding percent (5-80)
   */
  let { maxPaddingPercent = 30 } = $props();

  const frameSize = 100;
  const svgSize = 116;
  const frameOffset = (svgSize - frameSize) / 2;

  // A padding fraction of P% means up to P% of the area can be outside the
  // original photo. For a uniform border: area_fraction = 1 - inner_side².
  // So inner_side_fraction = sqrt(1 - P/100).
  const innerSideFraction = $derived(Math.sqrt(Math.max(0, 1 - maxPaddingPercent / 100)));
  const innerSize = $derived(frameSize * innerSideFraction);
  const border = $derived((frameSize - innerSize) / 2);

  const innerX = $derived(frameOffset + border);
  const innerY = $derived(frameOffset + border);
</script>

<div class="padding-indicator-wrap">
  <svg viewBox="0 0 {svgSize} {svgSize}" class="padding-svg" role="img" aria-label="Padding threshold visualization">
    <!-- Outer square: full output frame -->
    <rect
      x={frameOffset}
      y={frameOffset}
      width={frameSize}
      height={frameSize}
      fill="#2a2a2a"
      stroke="#666"
      stroke-width="1.5"
      rx="2"
    />

    <!-- Padding zone: red tint over the border area -->
    <rect
      x={frameOffset}
      y={frameOffset}
      width={frameSize}
      height={frameSize}
      fill="rgba(239, 68, 68, 0.30)"
      rx="2"
    />

    <!-- Inner square: real image pixels region -->
    {#if innerSize > 2}
      <rect
        x={innerX}
        y={innerY}
        width={innerSize}
        height={innerSize}
        fill="#3a3a3a"
      />
      <!-- Dashed border to distinguish inner from outer -->
      <rect
        x={innerX}
        y={innerY}
        width={innerSize}
        height={innerSize}
        fill="none"
        stroke="rgba(255,255,255,0.35)"
        stroke-width="1"
        stroke-dasharray="3 2"
      />
    {/if}
  </svg>

  <div class="legend">
    <div class="legend-item">
      <span class="swatch real"></span>
      real image pixels
    </div>
    <div class="legend-item">
      <span class="swatch padding"></span>
      padding (duplicated edge pixels)
    </div>
  </div>
</div>

<style>
  .padding-indicator-wrap {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 1rem;
  }

  .padding-svg {
    width: 116px;
    height: 116px;
    flex-shrink: 0;
  }

  .legend {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: #aaa;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .swatch {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 1px solid #666;
    border-radius: 2px;
    flex-shrink: 0;
  }

  .swatch.real {
    background: #3a3a3a;
  }

  .swatch.padding {
    background: color-mix(in srgb, rgb(239, 68, 68) 30%, #2a2a2a);
  }
</style>
