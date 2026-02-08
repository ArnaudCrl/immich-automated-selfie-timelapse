<script>
  /**
   * Visual indicator for alignment settings showing head position and scale.
   * Displays an anatomically proportioned face with eyes positioned according to eye_distance.
   *
   * The eye_distance parameter controls zoom level:
   * - Smaller values (0.1): zoomed out, head appears smaller, shows more neck
   * - Larger values (0.5): zoomed in, head appears larger, tighter crop
   *
   * Proportions match the processing pipeline (alignment.rs):
   * - Eye positions calculated with FACE_CENTER_BELOW_EYES = 0.5
   * - Face center (hairline-to-chin midpoint + neck) positioned at frame center
   * - This creates a vertical offset showing more neck than forehead
   * - Inter-pupillary distance (IPD) is ~37% of face width
   */
  let { eyeDistance = 0.3 } = $props();

  // Frame dimensions (square output image)
  const frameSize = 120;

  // Calculate eye positions using the same formulas as alignment.rs
  const FACE_CENTER_BELOW_EYES = 0.5;

  const leftEyeX = $derived((1.0 - eyeDistance) / 2.0);
  const rightEyeX = $derived(1.0 - leftEyeX);
  const eyeY = $derived(0.5 - eyeDistance * FACE_CENTER_BELOW_EYES);

  // Convert to pixel coordinates
  const leftEyePos = $derived({ x: leftEyeX * frameSize, y: eyeY * frameSize });
  const rightEyePos = $derived({ x: rightEyeX * frameSize, y: eyeY * frameSize });

  // Calculate eye distance in pixels
  const eyeDistancePx = $derived(eyeDistance * frameSize);

  // Calculate eye center point
  const eyeCenterX = $derived((leftEyePos.x + rightEyePos.x) / 2);
  const eyeCenterY = $derived(eyeY * frameSize);

  // Face proportions based on inter-pupillary distance (IPD)
  // Per crop_utils.rs, IPD is ~0.37 of face width
  // Face center (hairline-to-chin midpoint + neck) is 0.5 * IPD below eyes

  // Anatomical proportions relative to IPD:
  // - Eyes to top of head (forehead): ~0.5 IPD
  // - Eyes to face center: 0.5 IPD (by definition)
  // - Face center to bottom (lower face + neck): ~0.5 IPD
  // Total head height: ~1.5 IPD
  const foreheadHeight = $derived(eyeDistancePx * 0.5);
  const lowerFaceHeight = $derived(eyeDistancePx * 1.0);

  // Head ellipse dimensions (head is taller than wide)
  const headCenter = $derived({
    x: eyeCenterX,
    y: eyeCenterY + (lowerFaceHeight - foreheadHeight) / 2
  });
  const headWidth = $derived(eyeDistancePx * 1.35);  // Width relative to IPD
  const headHeight = $derived(eyeDistancePx * 1.85);  // Height > width (ratio ~1.37)

  // Eye rendering parameters (scale with IPD)
  const eyeRadius = $derived(Math.max(2.5, eyeDistancePx * 0.12));
</script>

<svg
  viewBox="0 0 {frameSize} {frameSize}"
  class="alignment-indicator"
  role="img"
  aria-label="Face alignment indicator"
>
  <!-- Background frame -->
  <rect
    x="0"
    y="0"
    width={frameSize}
    height={frameSize}
    fill="rgba(0, 0, 0, 0.8)"
    stroke="none"
  />

  <!-- Head ellipse (face shape) -->
  <ellipse
    cx={headCenter.x}
    cy={headCenter.y}
    rx={headWidth}
    ry={headHeight}
    fill="none"
    stroke="white"
    stroke-width="2"
    class="head"
  />

  <!-- Left eye -->
  <circle
    cx={leftEyePos.x}
    cy={leftEyePos.y}
    r={eyeRadius}
    fill="none"
    stroke="white"
    stroke-width="2"
    class="eye"
  />

  <!-- Right eye -->
  <circle
    cx={rightEyePos.x}
    cy={rightEyePos.y}
    r={eyeRadius}
    fill="none"
    stroke="white"
    stroke-width="2"
    class="eye"
  />

  <!-- Simple mouth curve (positioned between eyes and chin) -->
  <path
    d="M {eyeCenterX - eyeDistancePx * 0.35} {eyeCenterY + eyeDistancePx * 0.65}
       Q {eyeCenterX} {eyeCenterY + eyeDistancePx * 0.75},
         {eyeCenterX + eyeDistancePx * 0.35} {eyeCenterY + eyeDistancePx * 0.65}"
    fill="none"
    stroke="white"
    stroke-width="2"
    stroke-linecap="round"
    class="mouth"
  />
</svg>

<style>
  .alignment-indicator {
    width: 80px;
    height: 80px;
  }

  .head,
  .eye,
  .mouth {
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }
</style>
