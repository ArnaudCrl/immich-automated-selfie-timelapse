<script>
  let { jobStatus, progress } = $props();

  let percentage = $derived(
    progress.total > 0 ? Math.round((progress.completed / progress.total) * 100) : 0
  );

  let statusLabel = $derived({
    idle: 'Idle',
    running: 'Processing',
    cancelling: 'Cancelling',
    compiling_video: 'Compiling Video',
    completed: 'Completed',
    cancelled: 'Cancelled',
    error: 'Error',
  }[jobStatus] || jobStatus);

  let statusClass = $derived({
    idle: '',
    running: 'running',
    cancelling: 'warning',
    compiling_video: 'running',
    completed: 'success',
    cancelled: 'warning',
    error: 'error',
  }[jobStatus] || '');

  let canCancel = $derived(jobStatus === 'running' || jobStatus === 'compiling_video');

  // Person being processed
  let personDisplay = $derived(progress.person_name || progress.person_id || null);

  // Skip statistics from the backend
  let skipStats = $derived(progress.skip_stats || {
    face_too_small: 0,
    eyes_closed: 0,
    head_turned: 0,
    too_dark: 0,
    too_bright: 0,
    no_face_detected: 0,
    download_failed: 0,
    decode_failed: 0,
    crop_failed: 0,
    total: 0,
  });

  // Calculate kept (successful) count: completed - skipped
  let keptCount = $derived(Math.max(0, progress.completed - skipStats.total));

  // Filter to only show non-zero skip reasons
  let skipReasons = $derived(
    [
      { label: 'Face too small', count: skipStats.face_too_small },
      { label: 'Eyes closed', count: skipStats.eyes_closed },
      { label: 'Head turned', count: skipStats.head_turned },
      { label: 'Too dark', count: skipStats.too_dark },
      { label: 'Too bright', count: skipStats.too_bright },
      { label: 'No face detected', count: skipStats.no_face_detected },
      { label: 'Download failed', count: skipStats.download_failed },
      { label: 'Decode failed', count: skipStats.decode_failed },
      { label: 'Crop failed', count: skipStats.crop_failed },
    ].filter(r => r.count > 0)
  );

  async function cancelProcessing() {
    try {
      await fetch('/api/cancel', { method: 'POST' });
    } catch (e) {
      console.error('Cancel failed:', e);
    }
  }
</script>

<div class="progress-display">
  <div class="header">
    <div class="header-left">
      <span class="status {statusClass}">{statusLabel}</span>
      {#if personDisplay}
        <span class="person-name">{personDisplay}</span>
      {/if}
    </div>
    <div class="header-right">
      {#if progress.total > 0}
        <span class="count">{progress.completed} / {progress.total}</span>
      {/if}
      {#if canCancel}
        <button type="button" class="cancel-btn" onclick={cancelProcessing}>Cancel</button>
      {/if}
    </div>
  </div>

  {#if jobStatus === 'running' || jobStatus === 'compiling_video' || jobStatus === 'cancelling'}
    <div class="progress-bar">
      <div class="progress-fill" style="width: {percentage}%"></div>
    </div>
  {/if}

  {#if progress.message}
    <p class="message">{progress.message}</p>
  {/if}

  {#if (jobStatus === 'running' || jobStatus === 'completed' || jobStatus === 'cancelled') && progress.completed > 0}
    <div class="skip-stats">
      <div class="skip-header">
        <span class="skip-label">Discarded:Kept</span>
        <span class="skip-totals">
          <span class="skipped-count">{skipStats.total}</span>
          <span class="separator">:</span>
          <span class="kept-count">{keptCount}</span>
        </span>
      </div>
      <ul class="skip-reasons">
        {#each skipReasons as reason}
          <li class="skip-reason">
            <span class="reason-label">{reason.label}</span>
            <span class="reason-count">{reason.count}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .progress-display {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1.5rem;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .person-name {
    font-size: 0.875rem;
    color: #e0e0e0;
    font-weight: 500;
  }

  .status {
    font-weight: 600;
    font-size: 0.875rem;
    padding: 0.25rem 0.75rem;
    border-radius: 9999px;
    background: #333;
  }

  .status.running {
    background: #1e3a5f;
    color: #60a5fa;
  }

  .status.success {
    background: #14532d;
    color: #4ade80;
  }

  .status.warning {
    background: #422006;
    color: #fbbf24;
  }

  .status.error {
    background: #450a0a;
    color: #f87171;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .count {
    font-size: 0.875rem;
    color: #888;
  }

  .cancel-btn {
    padding: 0.375rem 0.75rem;
    background: #dc2626;
    border: none;
    border-radius: 4px;
    color: #fff;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .cancel-btn:hover {
    background: #b91c1c;
  }

  .progress-bar {
    height: 8px;
    background: #333;
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 1rem;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #4f46e5, #7c3aed);
    transition: width 0.3s ease;
  }

  .message {
    font-size: 0.875rem;
    color: #888;
  }

  /* Skip statistics */
  .skip-stats {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #252525;
  }

  .skip-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .skip-label {
    font-size: 0.75rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .skip-totals {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.875rem;
    font-weight: 600;
  }

  .skipped-count {
    color: #f59e0b;
  }

  .separator {
    color: #666;
  }

  .kept-count {
    color: #22c55e;
  }

  .skip-reasons {
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 0.25rem;
  }

  .skip-reason {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.375rem 0.5rem;
    background: #252525;
    border-radius: 4px;
    font-size: 0.75rem;
  }

  .reason-label {
    color: #888;
  }

  .reason-count {
    color: #e0e0e0;
    font-weight: 500;
  }
</style>
