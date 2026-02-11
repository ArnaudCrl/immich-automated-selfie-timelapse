<script>
  import { formatSize } from '../utils.js';
  import { showErrorAlert } from '../errorHandler.js';
  import { API } from '../constants.js';

  let { disabled = false, folders = [], onOpenGallery, onFolderDeleted } = $props();

  let deleting = $state(null);

  async function deleteFolder(name) {
    if (!confirm(`Delete output folder "${name}"? This cannot be undone.`)) {
      return;
    }

    deleting = name;
    try {
      const res = await fetch(`${API.output}/${encodeURIComponent(name)}`, {
        method: 'DELETE',
      });
      if (!res.ok) throw res;
      // Notify parent that folder was deleted (parent will reload)
      onFolderDeleted?.(name);
    } catch (e) {
      await showErrorAlert('Failed to delete folder', e);
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
      const res = await fetch(API.output, {
        method: 'DELETE',
      });
      if (!res.ok) throw res;
      // Notify parent that all folders were deleted (null = all)
      onFolderDeleted?.(null);
    } catch (e) {
      await showErrorAlert('Failed to delete folders', e);
    } finally {
      deleting = null;
    }
  }
</script>

<div class="output-manager">
  <div class="header">
    <h3>Output Folders</h3>
  </div>

  {#if folders.length === 0}
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
                href="/output/{encodeURIComponent(folder.name)}/{encodeURIComponent(folder.name)}.mp4"
                target="_blank"
                class="action-btn"
              >
                View
              </a>
              <a
                href="/output/{encodeURIComponent(folder.name)}/{encodeURIComponent(folder.name)}.mp4"
                download="{folder.name}.mp4"
                class="action-btn"
              >
                Download
              </a>
            {/if}
            <button
              type="button"
              class="gallery-btn"
              onclick={() => onOpenGallery?.(folder)}
              disabled={disabled || deleting !== null}
            >
              Gallery
            </button>
            <button
              type="button"
              class="delete-btn"
              aria-label="Delete folder {folder.name}"
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
        type="button"
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

  .action-btn {
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

  .action-btn:hover {
    background: #4f46e5;
  }

  .gallery-btn {
    padding: 0.375rem 0.75rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .gallery-btn:hover:not(:disabled) {
    background: #4f46e5;
  }

  .gallery-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
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
