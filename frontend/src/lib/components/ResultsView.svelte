<script>
  let { folderName = null } = $props();

  let cacheBust = $state(Date.now());
  let videoUrl = $derived(
    folderName
      ? `/output/${encodeURIComponent(folderName)}/${encodeURIComponent(folderName)}.mp4?v=${cacheBust}`
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
</style>
