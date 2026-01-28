<script>
  import { onMount, onDestroy } from 'svelte';

  let { personId, personName, jobStatus, onupdate } = $props();

  let dateFrom = $state('');
  let dateTo = $state('');
  let pollInterval = $state(null);
  let assetCount = $state(null);
  let loadingCount = $state(false);

  let isRunning = $derived(
    jobStatus === 'running' || jobStatus === 'compiling_video' || jobStatus === 'cancelling'
  );

  // Fetch asset count when personId changes
  $effect(() => {
    if (personId) {
      fetchAssetCount(personId);
    }
  });

  async function fetchAssetCount(id) {
    loadingCount = true;
    assetCount = null;
    try {
      const res = await fetch(`/api/people/${encodeURIComponent(id)}/asset-count`);
      if (res.ok) {
        assetCount = await res.json();
      }
    } catch (e) {
      console.error('Failed to fetch asset count:', e);
    } finally {
      loadingCount = false;
    }
  }

  async function startProcessing() {
    try {
      const res = await fetch('/api/start', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          person_id: personId,
          person_name: personName || null,
          date_from: dateFrom || null,
          date_to: dateTo || null,
        }),
      });

      const data = await res.json();

      if (res.ok && data.success) {
        startPolling();
      } else {
        onupdate?.({
          status: 'error',
          completed: 0,
          total: 0,
          message: data.message || 'Failed to start processing',
        });
      }
    } catch (e) {
      onupdate?.({
        status: 'error',
        completed: 0,
        total: 0,
        message: 'Network error: ' + e.message,
      });
    }
  }

  async function cancelProcessing() {
    try {
      await fetch('/api/cancel', { method: 'POST' });
    } catch (e) {
      console.error('Cancel failed:', e);
    }
  }

  async function pollProgress() {
    try {
      const res = await fetch('/api/progress');
      const data = await res.json();

      onupdate?.(data);

      if (data.status === 'completed' || data.status === 'cancelled' || data.status === 'error') {
        stopPolling();
      }
    } catch (e) {
      console.error('Poll failed:', e);
    }
  }

  function startPolling() {
    stopPolling();
    pollProgress();
    pollInterval = setInterval(pollProgress, 500);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  onMount(() => {
    // Check if there's already a running job
    pollProgress();
  });

  onDestroy(() => {
    stopPolling();
  });
</script>

<div class="processing-controls">
  <h2>Create Timelapse</h2>

  <div class="selected-person">
    <span class="person-name">
      Selected: <strong>{personName || 'Unnamed'}</strong>
    </span>
    {#if loadingCount}
      <span class="asset-count loading">Loading images...</span>
    {:else if assetCount}
      <span class="asset-count">
        <span class="count-number">{assetCount.assets_with_faces}</span> images with face data
        {#if assetCount.total_assets !== assetCount.assets_with_faces}
          <span class="count-detail">({assetCount.total_assets} total)</span>
        {/if}
      </span>
    {/if}
  </div>

  <div class="date-filters">
    <label>
      <span>From</span>
      <input type="date" bind:value={dateFrom} disabled={isRunning} />
    </label>
    <label>
      <span>To</span>
      <input type="date" bind:value={dateTo} disabled={isRunning} />
    </label>
  </div>

  <div class="actions">
    {#if isRunning}
      <button class="cancel-btn" onclick={cancelProcessing}>
        Cancel
      </button>
    {:else}
      <button class="start-btn" onclick={startProcessing}>
        Start Processing
      </button>
    {/if}
  </div>
</div>

<style>
  .processing-controls {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1.5rem;
    margin-top: 1rem;
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 1rem;
    color: #fff;
  }

  .selected-person {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 1rem;
  }

  .person-name {
    font-size: 0.875rem;
    color: #888;
  }

  .person-name strong {
    color: #e0e0e0;
  }

  .asset-count {
    font-size: 0.875rem;
    color: #4f46e5;
  }

  .asset-count.loading {
    color: #888;
    font-style: italic;
  }

  .count-number {
    font-weight: 600;
    font-size: 1rem;
  }

  .count-detail {
    color: #666;
    font-size: 0.75rem;
  }

  .date-filters {
    display: flex;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .date-filters label {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .date-filters span {
    font-size: 0.75rem;
    color: #888;
  }

  .date-filters input {
    padding: 0.75rem;
    border: 1px solid #333;
    border-radius: 6px;
    background: #0f0f0f;
    color: #e0e0e0;
    font-size: 0.875rem;
  }

  .date-filters input:focus {
    outline: none;
    border-color: #4f46e5;
  }

  .date-filters input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .actions {
    display: flex;
    gap: 1rem;
  }

  button {
    flex: 1;
    padding: 0.875rem 1.5rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .start-btn {
    background: #4f46e5;
    color: #fff;
  }

  .start-btn:hover {
    background: #4338ca;
  }

  .cancel-btn {
    background: #dc2626;
    color: #fff;
  }

  .cancel-btn:hover {
    background: #b91c1c;
  }
</style>
