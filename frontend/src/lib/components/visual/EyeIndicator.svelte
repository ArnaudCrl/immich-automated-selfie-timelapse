<script>
  /**
   * Minimalist eye graphic that opens/closes based on Eye Aspect Ratio (EAR)
   * Uses simple white lines on transparent background
   * EAR typically ranges from ~0.1 (closed) to ~0.4 (fully open)
   */
  let { ear = 0.2 } = $props();

  // Normalize EAR to a 0-1 range for animation
  // 0.1 = fully closed (0), 0.4 = fully open (1)
  const normalizedOpenness = $derived(Math.max(0, Math.min(1, (ear - 0.1) / 0.3)));

  // Eye dimensions
  const centerX = 50;
  const centerY = 30;
  const eyeWidth = 70;
  const maxEyeHeight = 35;
  const minEyeHeight = 2;

  // Calculate eye height based on openness
  const eyeHeight = $derived(minEyeHeight + normalizedOpenness * (maxEyeHeight - minEyeHeight));
  const pupilRadius = 2;

  // Create almond-shaped eye outline using a path
  // The path creates a pointed eye shape (yes, this is a Skyrim reference.)
  const eyePath = $derived.by(() => {
    const halfWidth = eyeWidth / 2;
    const halfHeight = eyeHeight / 2;

    // Start at left point, curve to top, to right point, curve to bottom, back to start
    return `M ${centerX - halfWidth} ${centerY}
            Q ${centerX - halfWidth/2} ${centerY - halfHeight}, ${centerX} ${centerY - halfHeight}
            Q ${centerX + halfWidth/2} ${centerY - halfHeight}, ${centerX + halfWidth} ${centerY}
            Q ${centerX + halfWidth/2} ${centerY + halfHeight}, ${centerX} ${centerY + halfHeight}
            Q ${centerX - halfWidth/2} ${centerY + halfHeight}, ${centerX - halfWidth} ${centerY}
            Z`;
  });
</script>

<svg
  viewBox="0 0 100 60"
  class="eye-indicator"
  role="img"
  aria-label="Eye openness indicator"
>
  <!-- Eye outline (almond shape) -->
  <path
    d={eyePath}
    fill="none"
    stroke="white"
    stroke-width="2.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    class="eye-outline"
  />

  <!-- Iris (circle outline) -->
  <circle
    cx={centerX}
    cy={centerY}
    r={eyeHeight / 2}
    fill="none"
    stroke="white"
    stroke-width="2"
    class="iris"
  />

  <!-- Pupil (filled circle) -->
  <circle
    cx={centerX}
    cy={centerY}
    r={Math.max(0, Math.min(pupilRadius, eyeHeight / 2 - 8))}
    fill="white"
    class="pupil"
  />
</svg>

<style>
  .eye-indicator {
    width: 80px;
    height: 48px;
  }

  .eye-outline,
  .iris,
  .pupil {
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }
</style>
