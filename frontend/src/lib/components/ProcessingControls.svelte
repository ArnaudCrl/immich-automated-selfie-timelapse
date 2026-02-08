<script>
  import { sanitizeFolderName, formatSize } from '../utils.js';

  let { personId, personName, jobStatus, outputFolders = [], onupdate } = $props();

  let dateFrom = $state('');
  let dateTo = $state('');
  let assetCount = $state(null);
  let loadingCount = $state(false);

  let isRunning = $derived(
    jobStatus === 'running' || jobStatus === 'compiling_video' || jobStatus === 'cancelling'
  );

  // Check if an output folder already exists for this person
  let existingFolder = $derived.by(() => {
    const expectedName = sanitizeFolderName(personName, personId);
    return outputFolders.find(f => f.name === expectedName);
  });

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
        // Notify parent of job start
        onupdate?.({
          status: 'running',
          completed: 0,
          total: 0,
          message: 'Starting...',
        });
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

  function handleStartClick() {
    if (existingFolder) {
      const folder = existingFolder;
      const message = `"${folder.name}" already has ${folder.image_count} images (${formatSize(folder.size_bytes)})${folder.has_video ? ' and a compiled video' : ''}.\n\nAll existing content in ${folder.name} will be permanently deleted.\n\nClick OK to continue.`;

      if (confirm(message)) {
        startProcessing();
      }
    } else {
      startProcessing();
    }
  }
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
    <button type="button" class="start-btn" onclick={handleStartClick}>
      Start Processing
    </button>
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

  .start-btn:hover:not(:disabled) {
    background: #4338ca;
  }

  .start-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
