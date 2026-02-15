<script>
  import silhouetteUrl from '../../assets/silhouette.png';

  let { eyeDistance = 0.3 } = $props();

  const frameSize = 120;

  // Scale the silouhette so it fills the frame proportionally to eyeDistance
  const imgSize = $derived(eyeDistance * frameSize * (1 / 0.12));  // Eye distance approximated to around 12% of the width.
  const imgX = $derived((frameSize - imgSize) / 2);
  const imgY = $derived(((frameSize * 1.4) - imgSize) / 2);  // slight offset to show more neck
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

  <!-- Silhouette image scaled to head bounding box -->
  <image
    href={silhouetteUrl}
    x={imgX}
    y={imgY}
    width={imgSize}
    height={imgSize}
    class="silhouette"
  />
</svg>

<style>
  .alignment-indicator {
    width: 80px;
    height: 80px;
  }

  .silhouette {
    filter: invert(1);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }
</style>
