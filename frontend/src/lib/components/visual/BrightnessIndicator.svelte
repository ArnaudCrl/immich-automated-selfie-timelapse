<script>
  /**
   * Interactive brightness filter control.
   * Displays a gradient from black to white with draggable markers.
   *
   * @param {number} minBrightness - Minimum accepted brightness (0-1)
   * @param {number} maxBrightness - Maximum accepted brightness (0-1)
   */
  let { minBrightness = $bindable(0.1), maxBrightness = $bindable(0.95) } = $props();

  const width = 240;
  const height = 80;
  const gradientBarHeight = 30;
  const gradientBarY = 15;

  // Convert brightness values (0-1) to x positions
  const minX = $derived(minBrightness * width);
  const maxX = $derived(maxBrightness * width);

  // Unique ID for SVG gradient to avoid collision with multiple instances
  const gradientId = `brightness-gradient-${Math.random().toString(36).slice(2, 9)}`;

  let dragging = $state(null); // 'min' | 'max' | null
  let svgElement = $state(null);

  function handlePointerDown(e, type) {
    e.preventDefault();
    dragging = type;
    svgElement.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e) {
    if (!dragging || !svgElement) return;

    const rect = svgElement.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const scaleX = width / rect.width;
    const actualX = x * scaleX;

    // Clamp to valid range
    let newValue = Math.max(0, Math.min(1, actualX / width));

    // Round to nearest 0.05
    newValue = Math.round(newValue * 20) / 20;

    if (dragging === 'min') {
      // Ensure min doesn't exceed max - 0.1
      minBrightness = Math.min(newValue, maxBrightness - 0.1);
    } else if (dragging === 'max') {
      // Ensure max doesn't go below min + 0.1
      maxBrightness = Math.max(newValue, minBrightness + 0.1);
    }
  }

  function handlePointerUp(e) {
    if (dragging && svgElement) {
      svgElement.releasePointerCapture(e.pointerId);
    }
    dragging = null;
  }
</script>

<svg
  bind:this={svgElement}
  viewBox="0 0 {width} {height}"
  class="brightness-indicator"
  class:dragging={dragging !== null}
  role="group"
  aria-label="Brightness range control"
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointerleave={handlePointerUp}
>
  <!-- Define gradient from black to white -->
  <defs>
    <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" style="stop-color:rgb(0,0,0);stop-opacity:1" />
      <stop offset="100%" style="stop-color:rgb(255,255,255);stop-opacity:1" />
    </linearGradient>
  </defs>

  <!-- Background gradient bar -->
  <rect
    x="0"
    y={gradientBarY}
    width={width}
    height={gradientBarHeight}
    fill="url(#{gradientId})"
    stroke="white"
    stroke-width="1.5"
    rx="3"
  />

  <!-- Rejected range overlays (darker areas) -->
  <!-- Left rejected area (too dark) -->
  <rect
    x="0"
    y={gradientBarY}
    width={minX}
    height={gradientBarHeight}
    fill="rgba(255, 0, 0, 0.3)"
    class="rejected-area"
  />

  <!-- Right rejected area (too bright) -->
  <rect
    x={maxX}
    y={gradientBarY}
    width={width - maxX}
    height={gradientBarHeight}
    fill="rgba(255, 0, 0, 0.3)"
    class="rejected-area"
  />

  <!-- Min brightness marker -->
  <g
    class="marker"
    class:active={dragging === 'min'}
    onpointerdown={(e) => handlePointerDown(e, 'min')}
    role="button"
    tabindex="0"
  >
    <!-- Invisible wider hit area for easier grabbing -->
    <rect
      x={minX - 10}
      y={gradientBarY - 10}
      width="20"
      height={gradientBarHeight + 20}
      fill="transparent"
      class="hit-area"
    />
    <!-- Vertical line -->
    <line
      x1={minX}
      y1={gradientBarY - 5}
      x2={minX}
      y2={gradientBarY + gradientBarHeight + 5}
      stroke="#4f46e5"
      stroke-width="2.5"
      stroke-linecap="round"
    />
    <!-- Triangle at top -->
    <path
      d="M {minX - 4} {gradientBarY - 5} L {minX + 4} {gradientBarY - 5} L {minX} {gradientBarY - 1} Z"
      fill="#4f46e5"
    />
    <!-- Triangle at bottom -->
    <path
      d="M {minX - 4} {gradientBarY + gradientBarHeight + 5} L {minX + 4} {gradientBarY + gradientBarHeight + 5} L {minX} {gradientBarY + gradientBarHeight + 1} Z"
      fill="#4f46e5"
    />
  </g>

  <!-- Max brightness marker -->
  <g
    class="marker"
    class:active={dragging === 'max'}
    onpointerdown={(e) => handlePointerDown(e, 'max')}
    role="button"
    tabindex="0"
  >
    <!-- Invisible wider hit area for easier grabbing -->
    <rect
      x={maxX - 10}
      y={gradientBarY - 10}
      width="20"
      height={gradientBarHeight + 20}
      fill="transparent"
      class="hit-area"
    />
    <!-- Vertical line -->
    <line
      x1={maxX}
      y1={gradientBarY - 5}
      x2={maxX}
      y2={gradientBarY + gradientBarHeight + 5}
      stroke="#4f46e5"
      stroke-width="2.5"
      stroke-linecap="round"
    />
    <!-- Triangle at top -->
    <path
      d="M {maxX - 4} {gradientBarY - 5} L {maxX + 4} {gradientBarY - 5} L {maxX} {gradientBarY - 1} Z"
      fill="#4f46e5"
    />
    <!-- Triangle at bottom -->
    <path
      d="M {maxX - 4} {gradientBarY + gradientBarHeight + 5} L {maxX + 4} {gradientBarY + gradientBarHeight + 5} L {maxX} {gradientBarY + gradientBarHeight + 1} Z"
      fill="#4f46e5"
    />
  </g>

  <!-- Value labels -->
  <text
    x={minX}
    y={gradientBarY + gradientBarHeight + 20}
    text-anchor="middle"
    class="value-label"
  >
    {minBrightness.toFixed(2)}
  </text>
  <text
    x={maxX}
    y={gradientBarY + gradientBarHeight + 20}
    text-anchor="middle"
    class="value-label"
  >
    {maxBrightness.toFixed(2)}
  </text>
</svg>

<style>
  .brightness-indicator {
    width: 240px;
    height: 80px;
    touch-action: none;
    user-select: none;
  }

  .brightness-indicator.dragging {
    cursor: grabbing;
  }

  .marker {
    cursor: grab;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .marker:hover line,
  .marker:hover path {
    stroke: #6366f1;
    fill: #6366f1;
  }

  .marker.active {
    cursor: grabbing;
  }

  .marker.active line,
  .marker.active path {
    stroke: #818cf8;
    fill: #818cf8;
  }

  .hit-area {
    cursor: grab;
  }

  .rejected-area {
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .value-label {
    fill: #4f46e5;
    font-size: 14px;
    font-weight: 600;
    font-family: monospace;
    pointer-events: none;
    user-select: none;
  }
</style>
