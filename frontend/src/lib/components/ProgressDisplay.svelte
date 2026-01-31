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

  // Convert snake_case to readable label (e.g., "face_too_small" -> "Face too small")
  function formatLabel(key) {
    return key
      .split('_')
      .map((word, i) => i === 0 ? word.charAt(0).toUpperCase() + word.slice(1) : word)
      .join(' ');
  }

  // Skip statistics from the backend - dynamically handle any keys
  let skipStats = $derived(progress.skip_stats || {});

  // Calculate total from all numeric values in skip_stats
  let skipTotal = $derived(
    Object.values(skipStats).reduce((sum, val) => sum + (typeof val === 'number' ? val : 0), 0)
  );

  // Calculate kept (successful) count: completed - skipped
  let keptCount = $derived(Math.max(0, progress.completed - skipTotal));

  // Dynamically build skip reasons from whatever the backend sends
  // Filter to only show non-zero counts
  let skipReasons = $derived(
    Object.entries(skipStats)
      .filter(([_, count]) => typeof count === 'number' && count > 0)
      .map(([key, count]) => ({ label: formatLabel(key), count }))
      .sort((a, b) => b.count - a.count) // Sort by count descending
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
          <span class="skipped-count">{skipTotal}</span>
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
