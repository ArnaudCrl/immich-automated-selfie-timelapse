<script>
  import { sanitizeFolderName, formatSize } from '../utils.js';
  import { JOB_STATUS, API } from '../constants.js';
  import { handleError } from '../errorHandler.js';

  let { personId, personName, albums = [], jobStatus, outputFolders = [], onupdate } = $props();

  let dateFrom = $state('');
  let dateTo = $state('');
  let assetCount = $state(null);
  let loadingCount = $state(false);
  let starting = $state(false);
  let fetchSeq = 0;

  let isRunning = $derived(
    jobStatus === JOB_STATUS.running || jobStatus === JOB_STATUS.compiling || jobStatus === JOB_STATUS.cancelling
  );

  // Check if an output folder already exists for this person
  let existingFolder = $derived.by(() => {
    const expectedName = sanitizeFolderName(personName, personId);
    return outputFolders.find(f => f.name === expectedName);
  });

  // Fetch asset count when personId or albums changes
  $effect(() => {
    if (personId) {
      fetchAssetCount(personId, albums);
    }
  });

  async function fetchAssetCount(id, albumList = []) {
    const seq = ++fetchSeq;
    loadingCount = true;
    assetCount = null;
    try {
      let url = `${API.people}/${encodeURIComponent(id)}/asset-count`;
      if (albumList.length > 0) {
        url += `?album_ids=${albumList.map(a => encodeURIComponent(a.id)).join(',')}`;
      }
      const res = await fetch(url);
      if (!res.ok) throw res;
      const data = await res.json();
      if (seq === fetchSeq) {
        assetCount = data;
      }
    } catch (e) {
      if (seq === fetchSeq) {
        await handleError('Failed to fetch asset count', e);
      }
    } finally {
      if (seq === fetchSeq) {
        loadingCount = false;
      }
    }
  }

  async function startProcessing() {
    starting = true;
    try {
      const res = await fetch(API.start, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          person_id: personId,
          person_name: personName || null,
          date_from: dateFrom || null,
          date_to: dateTo || null,
          album_ids: albums.map(a => a.id),
          album_names: albums.map(a => a.name),
        }),
      });

      const data = await res.json();

      if (res.ok && data.success) {
        // Notify parent of job start
        onupdate?.({
          status: JOB_STATUS.running,
          completed: 0,
          total: 0,
          message: 'Starting...',
        });
      } else {
        onupdate?.({
          status: JOB_STATUS.error,
          completed: 0,
          total: 0,
          message: data.message || 'Failed to start processing',
        });
      }
    } catch (e) {
      const errorMessage = await handleError('Failed to start processing', e);
      onupdate?.({
        status: JOB_STATUS.error,
        completed: 0,
        total: 0,
        message: errorMessage,
      });
    } finally {
      starting = false;
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
      Person: <strong>{personName || 'Unnamed'}</strong>
    </span>
    {#if albums.length === 1}
      <span class="album-name">
        Album: <strong>{albums[0].name}</strong>
      </span>
    {:else if albums.length > 1}
      <span class="album-name">
        Albums: <strong>{albums.length} selected</strong>
      </span>
    {/if}
    {#if loadingCount}
      <span class="asset-count loading">Loading images...</span>
    {:else if assetCount}
      <span class="asset-count">
        <span class="count-number">{assetCount.assets_with_faces}</span> images of <strong>{personName || 'Unnamed'}</strong>{albums.length === 1 ? ' in this album' : albums.length > 1 ? ` across ${albums.length} albums` : ''}
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
    <button type="button" class="start-btn" onclick={handleStartClick} disabled={starting || isRunning}>
      {starting ? 'Starting...' : 'Start Processing'}
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

  .person-name, .album-name {
    font-size: 0.875rem;
    color: #888;
  }

  .person-name strong, .album-name strong {
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
