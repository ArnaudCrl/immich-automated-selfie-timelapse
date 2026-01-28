<script>
  import { onMount } from 'svelte';

  let { disabled = false } = $props();

  let folders = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let deleting = $state(null);

  async function loadFolders() {
    loading = true;
    error = null;
    try {
      const res = await fetch('/api/output');
      if (!res.ok) throw new Error('Failed to load output folders');
      folders = await res.json();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function deleteFolder(name) {
    if (!confirm(`Delete output folder "${name}"? This cannot be undone.`)) {
      return;
    }

    deleting = name;
    try {
      const res = await fetch(`/api/output/${encodeURIComponent(name)}`, {
        method: 'DELETE',
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || 'Failed to delete folder');
      }
      // Reload the list
      await loadFolders();
    } catch (e) {
      alert(e.message);
    } finally {
      deleting = null;
    }
  }

  async function deleteAll() {
    if (!confirm('Delete ALL output folders? This cannot be undone.')) {
      return;
    }

    deleting = '__all__';
    try {
      const res = await fetch('/api/output', {
        method: 'DELETE',
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || 'Failed to delete folders');
      }
      // Reload the list
      await loadFolders();
    } catch (e) {
      alert(e.message);
    } finally {
      deleting = null;
    }
  }

  function formatSize(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  onMount(() => {
    loadFolders();
  });
</script>

<div class="output-manager">
  <div class="header">
    <h3>Output Folders</h3>
    <button class="refresh-btn" onclick={loadFolders} disabled={loading || disabled}>
      {loading ? '...' : '↻'}
    </button>
  </div>

  {#if loading}
    <p class="status">Loading...</p>
  {:else if error}
    <p class="error">{error}</p>
  {:else if folders.length === 0}
    <p class="status empty">No output folders yet</p>
  {:else}
    <ul class="folder-list">
      {#each folders as folder}
        <li class="folder-item">
          <div class="folder-info">
            <span class="folder-name">{folder.name}</span>
            <span class="folder-stats">
              {folder.image_count} images • {formatSize(folder.size_bytes)}
              {#if folder.has_video}
                <span class="video-badge">Video</span>
              {/if}
            </span>
          </div>
          <div class="folder-actions">
            {#if folder.has_video}
              <a
                href="/output/{encodeURIComponent(folder.name)}/timelapse.mp4"
                target="_blank"
                class="view-btn"
              >
                View
              </a>
            {/if}
            <button
              class="delete-btn"
              onclick={() => deleteFolder(folder.name)}
              disabled={disabled || deleting !== null}
            >
              {deleting === folder.name ? '...' : '×'}
            </button>
          </div>
        </li>
      {/each}
    </ul>

    <div class="actions">
      <button
        class="delete-all-btn"
        onclick={deleteAll}
        disabled={disabled || deleting !== null || folders.length === 0}
      >
        {deleting === '__all__' ? 'Deleting...' : 'Delete All'}
      </button>
    </div>
  {/if}
</div>

<style>
  .output-manager {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1rem 1.5rem;
    margin-bottom: 1rem;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  h3 {
    font-size: 0.875rem;
    font-weight: 600;
    color: #e0e0e0;
    margin: 0;
  }

  .refresh-btn {
    width: 2rem;
    height: 2rem;
    background: #252525;
    border: none;
    border-radius: 4px;
    color: #888;
    font-size: 1rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .refresh-btn:hover:not(:disabled) {
    background: #333;
    color: #e0e0e0;
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .status {
    font-size: 0.875rem;
    color: #888;
    text-align: center;
    padding: 1rem;
  }

  .status.empty {
    color: #666;
    font-style: italic;
  }

  .error {
    font-size: 0.875rem;
    color: #dc2626;
    text-align: center;
    padding: 1rem;
  }

  .folder-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .folder-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0;
    border-bottom: 1px solid #252525;
  }

  .folder-item:last-child {
    border-bottom: none;
  }

  .folder-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .folder-name {
    font-size: 0.875rem;
    color: #e0e0e0;
    font-weight: 500;
  }

  .folder-stats {
    font-size: 0.75rem;
    color: #666;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .video-badge {
    background: #22c55e;
    color: #0f0f0f;
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 2px;
    text-transform: uppercase;
  }

  .folder-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .view-btn {
    padding: 0.375rem 0.75rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 0.75rem;
    text-decoration: none;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .view-btn:hover {
    background: #4f46e5;
  }

  .delete-btn {
    width: 2rem;
    height: 2rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #888;
    font-size: 1.25rem;
    cursor: pointer;
    transition: all 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .delete-btn:hover:not(:disabled) {
    background: #dc2626;
    color: #fff;
  }

  .delete-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .actions {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #252525;
  }

  .delete-all-btn {
    padding: 0.5rem 1rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #888;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .delete-all-btn:hover:not(:disabled) {
    background: #dc2626;
    color: #fff;
  }

  .delete-all-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
