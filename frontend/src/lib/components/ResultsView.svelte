<script>
  // Future: This component can be extended to show:
  // - Preview grid of processed faces
  // - Before/after alignment comparison
  // - Skipped images with reasons
  // - Face landmark visualization

  let { folderName = null } = $props();

  let videoUrl = $derived(
    folderName
      ? `/output/${encodeURIComponent(folderName)}/${encodeURIComponent(folderName)}.mp4`
      : null
  );
  let videoError = $state(false);

  function handleVideoError() {
    videoError = true;
  }
</script>

<div class="results-view">
  <h2>Result</h2>

  {#if !videoUrl}
    <div class="video-error">
      <p>No video available.</p>
      <p class="hint">Person information was not provided.</p>
    </div>
  {:else if videoError}
    <div class="video-error">
      <p>Video not available yet.</p>
      <p class="hint">The video file may still be processing or the path may have changed.</p>
    </div>
  {:else}
    <div class="video-container">
      <video
        controls
        autoplay
        loop
        src={videoUrl}
        onerror={handleVideoError}
      >
        <track kind="captions" />
        Your browser does not support the video tag.
      </video>
    </div>

    <div class="actions">
      <a href={videoUrl} download="{folderName}.mp4" class="download-btn">
        Download Video
      </a>
    </div>
  {/if}

  <!-- Future: Face preview grid -->
  <!--
  <div class="preview-section">
    <h3>Processed Faces</h3>
    <div class="preview-grid">
      {#each processedFaces as face}
        <div class="face-preview">
          <img src={face.url} alt="Processed face" />
          <span class="timestamp">{face.timestamp}</span>
        </div>
      {/each}
    </div>
  </div>
  -->

  <!-- Future: Skipped images -->
  <!--
  <details class="skipped-section">
    <summary>Skipped Images ({skippedCount})</summary>
    <ul>
      {#each skippedImages as img}
        <li>{img.reason}</li>
      {/each}
    </ul>
  </details>
  -->
</div>

<style>
  .results-view {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1.5rem;
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 1rem;
    color: #fff;
  }

  .video-container {
    position: relative;
    border-radius: 8px;
    overflow: hidden;
    background: #000;
    margin-bottom: 1rem;
  }

  video {
    width: 100%;
    display: block;
  }

  .video-error {
    text-align: center;
    padding: 3rem;
    background: #252525;
    border-radius: 8px;
  }

  .video-error p {
    margin-bottom: 0.5rem;
  }

  .hint {
    font-size: 0.875rem;
    color: #666;
  }

  .actions {
    display: flex;
    gap: 1rem;
  }

  .download-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1.5rem;
    background: #22c55e;
    color: #fff;
    text-decoration: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 600;
    transition: background 0.15s ease;
  }

  .download-btn:hover {
    background: #16a34a;
  }

  /* Future styles for preview grid */
  /*
  .preview-section {
    margin-top: 2rem;
    padding-top: 1.5rem;
    border-top: 1px solid #333;
  }

  .preview-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    gap: 0.5rem;
  }

  .face-preview img {
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    border-radius: 4px;
  }

  .skipped-section {
    margin-top: 1rem;
    font-size: 0.875rem;
    color: #888;
  }
  */
</style>
