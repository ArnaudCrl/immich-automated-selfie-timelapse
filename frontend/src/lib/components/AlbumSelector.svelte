<script>
  import { onMount } from 'svelte';
  import { handleError } from '../errorHandler.js';
  import { API } from '../constants.js';

  let { disabled = false, onselect, initialSelectedIds = [] } = $props();

  let albums = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let selectedIds = $state(new Set());

  async function loadAlbums() {
    loading = true;
    error = null;

    try {
      const res = await fetch(API.albums);
      if (!res.ok) throw res;
      albums = await res.json();

      // Restore selection from initialSelectedIds, filtering to albums that still exist
      const validIds = new Set(albums.map(a => a.id));
      const restored = initialSelectedIds.filter(id => validIds.has(id));
      selectedIds = new Set(restored);
      onselect?.(albums.filter(a => selectedIds.has(a.id)));
    } catch (e) {
      error = await handleError('Failed to load albums', e);
    } finally {
      loading = false;
    }
  }

  function selectAlbum(album) {
    if (disabled) return;
    const next = new Set(selectedIds);
    if (next.has(album.id)) {
      next.delete(album.id);
    } else {
      next.add(album.id);
    }
    selectedIds = next;
    onselect?.(albums.filter(a => selectedIds.has(a.id)));
  }

  function clearSelection() {
    if (disabled) return;
    selectedIds = new Set();
    onselect?.([]);
  }

  onMount(() => {
    loadAlbums();
  });
</script>

<div class="album-selector">
  <div class="header">
    <h2>Filter by Album <span class="optional">(optional)</span></h2>
    {#if selectedIds.size > 0}
      <span class="selected-count">{selectedIds.size} selected</span>
      <button type="button" class="clear-btn" onclick={clearSelection} {disabled}>
        Clear
      </button>
    {/if}
  </div>

  {#if loading}
    <div class="loading">Loading albums...</div>
  {:else if error}
    <div class="error">
      {error}
      <button type="button" onclick={loadAlbums}>Retry</button>
    </div>
  {:else if albums.length === 0}
    <p class="no-results">No albums found</p>
  {:else}
    <div class="albums-grid">
      {#each albums as album (album.id)}
        <button
          type="button"
          class="album-card"
          class:selected={selectedIds.has(album.id)}
          onclick={() => selectAlbum(album)}
          {disabled}
        >
          {#if selectedIds.has(album.id)}
            <span class="checkmark">✓</span>
          {/if}
          <div class="thumbnail">
            {#if album.thumbnail_asset_id}
              <img
                src="/api/albums/{encodeURIComponent(album.thumbnail_asset_id)}/thumbnail"
                alt={album.name}
                onerror={(e) => e.target.style.display = 'none'}
              />
            {/if}
            <span class="fallback">&#128193;</span>
          </div>
          <span class="name">{album.name}</span>
          <span class="count">{album.asset_count} photos</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .album-selector {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1.5rem;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
  }

  .optional {
    font-size: 0.75rem;
    font-weight: 400;
    color: #666;
  }

  .selected-count {
    font-size: 0.75rem;
    color: #4f46e5;
    margin-right: 0.5rem;
  }

  .checkmark {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 18px;
    height: 18px;
    background: #4f46e5;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.6rem;
    color: #fff;
    z-index: 2;
    line-height: 1;
  }

  .clear-btn {
    padding: 0.25rem 0.75rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #ccc;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .clear-btn:hover:not(:disabled) {
    background: #444;
    color: #fff;
  }

  .clear-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .albums-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 0.75rem;
    max-height: 300px;
    overflow-y: auto;
  }

  .album-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    padding: 1rem;
    background: #252525;
    border: 2px solid transparent;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
    position: relative;
  }

  .album-card:hover:not(:disabled) {
    background: #303030;
  }

  .album-card.selected {
    border-color: #4f46e5;
    background: #2a2a4a;
  }

  .album-card:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .thumbnail {
    width: 48px;
    height: 48px;
    border-radius: 6px;
    background: linear-gradient(135deg, #374151, #4b5563);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    overflow: hidden;
    position: relative;
  }

  .thumbnail img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    position: absolute;
    top: 0;
    left: 0;
    z-index: 1;
  }

  .fallback {
    z-index: 0;
  }

  .name {
    font-size: 0.75rem;
    color: #ccc;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .count {
    font-size: 0.625rem;
    color: #666;
    text-align: center;
  }

  .loading, .error, .no-results {
    text-align: center;
    padding: 2rem;
    color: #888;
  }

  .error {
    color: #dc2626;
  }

  .error button {
    margin-top: 0.5rem;
    padding: 0.5rem 1rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #fff;
    cursor: pointer;
  }
</style>
