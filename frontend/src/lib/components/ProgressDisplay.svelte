<script>
  let { jobStatus, progress } = $props();

  let percentage = $derived(
    progress.total > 0 ? Math.round((progress.completed / progress.total) * 100) : 0
  );

  let statusLabel = $derived({
    idle: 'Idle',
    running: 'Processing',
    compiling_video: 'Compiling Video',
    completed: 'Completed',
    cancelled: 'Cancelled',
    error: 'Error',
  }[jobStatus] || jobStatus);

  let statusClass = $derived({
    idle: '',
    running: 'running',
    compiling_video: 'running',
    completed: 'success',
    cancelled: 'warning',
    error: 'error',
  }[jobStatus] || '');
</script>

<div class="progress-display">
  <div class="header">
    <span class="status {statusClass}">{statusLabel}</span>
    {#if progress.total > 0}
      <span class="count">{progress.completed} / {progress.total}</span>
    {/if}
  </div>

  {#if jobStatus === 'running' || jobStatus === 'compiling_video'}
    <div class="progress-bar">
      <div class="progress-fill" style="width: {percentage}%"></div>
    </div>
  {/if}

  {#if progress.message}
    <p class="message">{progress.message}</p>
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

  .count {
    font-size: 0.875rem;
    color: #888;
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
</style>
