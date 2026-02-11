<script>
  import { onMount } from 'svelte';
  import { handleError, showErrorAlert } from '../errorHandler.js';
  import { API } from '../constants.js';

  let {
    folderName,
    onBack,
    disabled = false
  } = $props();

  let images = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let selectedImages = $state(new Set());
  let deleting = $state(false);
  let compiling = $state(false);
  let videoExists = $state(false);
  let cacheBust = $state(Date.now());

  let lightboxIndex = $state(null);
  let lightboxImage = $derived(lightboxIndex !== null ? images[lightboxIndex] : null);

  let selectedCount = $derived(selectedImages.size);
  let allSelected = $derived(images.length > 0 && selectedImages.size === images.length);

  function openLightbox(index) {
    lightboxIndex = index;
  }

  function closeLightbox() {
    lightboxIndex = null;
  }

  function lightboxPrev() {
    if (lightboxIndex > 0) lightboxIndex--;
  }

  function lightboxNext() {
    if (lightboxIndex < images.length - 1) lightboxIndex++;
  }

  function handleLightboxKeydown(e) {
    if (lightboxIndex === null) return;
    if (e.key === 'Escape') closeLightbox();
    else if (e.key === 'ArrowLeft') lightboxPrev();
    else if (e.key === 'ArrowRight') lightboxNext();
  }

  async function loadImages() {
    loading = true;
    error = null;
    cacheBust = Date.now();
    try {
      const res = await fetch(`${API.output}/${encodeURIComponent(folderName)}/images`);
      if (!res.ok) throw res;
      const data = await res.json();
      images = data.images;
      videoExists = data.video_exists;
      // Clear selection when reloading
      selectedImages = new Set();
    } catch (e) {
      error = await handleError('Failed to load images', e);
    } finally {
      loading = false;
    }
  }

  function toggleSelect(filename) {
    if (selectedImages.has(filename)) {
      selectedImages.delete(filename);
      selectedImages = new Set(selectedImages);
    } else {
      selectedImages.add(filename);
      selectedImages = new Set(selectedImages);
    }
  }

  function selectAll() {
    selectedImages = new Set(images.map(img => img.filename));
  }

  function deselectAll() {
    selectedImages = new Set();
  }

  async function deleteSelected() {
    if (selectedImages.size === 0) return;

    const count = selectedImages.size;
    if (!confirm(`Delete ${count} selected image${count > 1 ? 's' : ''}? This cannot be undone.`)) {
      return;
    }

    deleting = true;
    try {
      const res = await fetch(`${API.output}/${encodeURIComponent(folderName)}/images`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ filenames: Array.from(selectedImages) })
      });

      if (!res.ok) throw res;

      const result = await res.json();

      if (result.failed_count > 0) {
        alert(`Deleted ${result.deleted_count} images. ${result.failed_count} failed.`);
      }

      // Reload images
      await loadImages();
    } catch (e) {
      await showErrorAlert('Failed to delete images', e);
    } finally {
      deleting = false;
    }
  }

  async function compileVideo() {
    const message = videoExists
      ? 'Compile video from these images? This will overwrite the previous video.'
      : 'Compile video from these images?';
    if (!confirm(message)) {
      return;
    }

    compiling = true;
    try {
      const res = await fetch(`${API.output}/${encodeURIComponent(folderName)}/compile`, {
        method: 'POST'
      });

      if (!res.ok) throw res;

      // The compilation runs in background - return to main view to see progress
      alert('Video compilation started. Return to main view to see progress.');
      onBack?.();
    } catch (e) {
      await showErrorAlert('Failed to start video compilation', e);
      compiling = false;
    }
  }

  onMount(() => {
    loadImages();
  });
</script>

<div class="gallery-view">
  <header class="gallery-header">
    <button type="button" class="back-btn" onclick={onBack} disabled={disabled || deleting || compiling}>
      &larr; Back
    </button>
    <h2>
      {folderName}
      {#if !loading}
        <span class="image-count">({images.length} images)</span>
      {/if}
    </h2>
  </header>

  {#if loading}
    <div class="status">Loading images...</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if images.length === 0}
    <div class="status empty">No images in this folder</div>
  {:else}
    <div class="toolbar">
      <div class="selection-controls">
        <button
          type="button"
          class="toolbar-btn"
          onclick={selectAll}
          disabled={disabled || deleting || compiling || allSelected}
        >
          Select All
        </button>
        <button
          type="button"
          class="toolbar-btn"
          onclick={deselectAll}
          disabled={disabled || deleting || compiling || selectedCount === 0}
        >
          Deselect All
        </button>
        <button
          type="button"
          class="toolbar-btn delete-btn"
          onclick={deleteSelected}
          disabled={disabled || deleting || compiling || selectedCount === 0}
        >
          {#if deleting}
            Deleting...
          {:else}
            Delete Selected ({selectedCount})
          {/if}
        </button>
      </div>
    </div>

    <div class="image-grid">
      {#each images as image, i}
        <div
          class="image-card"
          class:selected={selectedImages.has(image.filename)}
        >
          <button
            type="button"
            class="checkbox-overlay"
            aria-label={selectedImages.has(image.filename) ? `Deselect ${image.filename}` : `Select ${image.filename}`}
            onclick={(e) => { e.stopPropagation(); toggleSelect(image.filename); }}
          >
            <span class="checkbox">{selectedImages.has(image.filename) ? '✓' : ''}</span>
          </button>
          <button
            type="button"
            class="image-button"
            onclick={() => openLightbox(i)}
          >
            <img
              src="/output/{encodeURIComponent(folderName)}/images/{encodeURIComponent(image.filename)}?v={cacheBust}"
              alt={image.filename}
              loading="lazy"
            />
          </button>
          <div class="image-info">
            <span class="image-date">{image.filename.split('_')[0]}</span>
          </div>
        </div>
      {/each}
    </div>

    <footer class="gallery-footer">
      <div class="footer-info">
        {selectedCount} selected
        {#if videoExists}
          <span class="video-badge">Video exists</span>
        {/if}
      </div>
      <button
        type="button"
        class="compile-btn"
        onclick={compileVideo}
        disabled={disabled || deleting || compiling || images.length === 0}
      >
        {#if compiling}
          Starting...
        {:else}
          Compile Video
        {/if}
      </button>
    </footer>
  {/if}
</div>

{#if lightboxImage}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="lightbox-overlay" onclick={closeLightbox} role="presentation">
    <button type="button" class="lightbox-close" onclick={closeLightbox}>&times;</button>

    {#if lightboxIndex > 0}
      <button type="button" class="lightbox-nav lightbox-prev" onclick={(e) => { e.stopPropagation(); lightboxPrev(); }}>&lsaquo;</button>
    {/if}

    <div class="lightbox-img-wrapper" role="presentation" onclick={(e) => e.stopPropagation()}>
      <img
        class="lightbox-img"
        src="/output/{encodeURIComponent(folderName)}/images/{encodeURIComponent(lightboxImage.filename)}?v={cacheBust}"
        alt={lightboxImage.filename}
      />
    </div>

    {#if lightboxIndex < images.length - 1}
      <button type="button" class="lightbox-nav lightbox-next" onclick={(e) => { e.stopPropagation(); lightboxNext(); }}>&rsaquo;</button>
    {/if}

    <div class="lightbox-info" role="presentation" onclick={(e) => e.stopPropagation()}>
      <span>{lightboxImage.filename}</span>
      <span class="lightbox-counter">{lightboxIndex + 1} / {images.length}</span>
    </div>
  </div>
{/if}

<svelte:window onkeydown={handleLightboxKeydown} />

<style>
  .gallery-view {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1.5rem;
    min-height: 400px;
    display: flex;
    flex-direction: column;
  }

  .gallery-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #333;
  }

  .back-btn {
    padding: 0.5rem 1rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .back-btn:hover:not(:disabled) {
    background: #444;
  }

  .back-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  h2 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #fff;
    margin: 0;
  }

  .image-count {
    font-weight: 400;
    color: #888;
    font-size: 0.875rem;
  }

  .status {
    text-align: center;
    padding: 3rem;
    color: #888;
    font-size: 0.875rem;
  }

  .status.empty {
    color: #666;
    font-style: italic;
  }

  .error {
    text-align: center;
    padding: 3rem;
    color: #dc2626;
    font-size: 0.875rem;
  }

  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .selection-controls {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .toolbar-btn {
    padding: 0.5rem 0.75rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: #444;
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toolbar-btn.delete-btn:hover:not(:disabled) {
    background: #dc2626;
    color: #fff;
  }

  .image-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 0.75rem;
    flex: 1;
    overflow-y: auto;
    padding: 0.25rem;
  }

  .image-card {
    position: relative;
    aspect-ratio: 1;
    border-radius: 4px;
    overflow: hidden;
    cursor: pointer;
    border: 2px solid transparent;
    transition: all 0.15s ease;
    background: #252525;
  }

  .image-card:hover {
    border-color: #444;
  }

  .image-card.selected {
    border-color: #4f46e5;
  }

  .image-card:focus {
    outline: 2px solid #4f46e5;
    outline-offset: 2px;
  }

  .checkbox-overlay {
    all: unset;
    position: absolute;
    top: 0.25rem;
    left: 0.25rem;
    width: 1.25rem;
    height: 1.25rem;
    background: rgba(0, 0, 0, 0.6);
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1;
    cursor: pointer;
  }

  .image-card.selected .checkbox-overlay {
    background: #4f46e5;
  }

  .checkbox {
    color: #fff;
    font-size: 0.75rem;
    font-weight: bold;
  }

  .image-card img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .image-info {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 0.25rem 0.5rem;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.8));
  }

  .image-date {
    font-size: 0.625rem;
    color: #ccc;
  }

  .gallery-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #333;
  }

  .footer-info {
    font-size: 0.875rem;
    color: #888;
    display: flex;
    align-items: center;
    gap: 0.75rem;
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

  .compile-btn {
    padding: 0.625rem 1.25rem;
    background: #4f46e5;
    border: none;
    border-radius: 4px;
    color: #fff;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .compile-btn:hover:not(:disabled) {
    background: #4338ca;
  }

  .compile-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .image-button {
    all: unset;
    display: block;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }

  .image-button img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .lightbox-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .lightbox-close {
    position: absolute;
    top: 1rem;
    right: 1rem;
    background: none;
    border: none;
    color: #fff;
    font-size: 2rem;
    cursor: pointer;
    padding: 0.5rem;
    line-height: 1;
    z-index: 1001;
    opacity: 0.7;
    transition: opacity 0.15s;
  }

  .lightbox-close:hover {
    opacity: 1;
  }

  .lightbox-nav {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    background: rgba(255, 255, 255, 0.1);
    border: none;
    color: #fff;
    font-size: 3rem;
    cursor: pointer;
    padding: 0.5rem 1rem;
    line-height: 1;
    z-index: 1001;
    opacity: 0.7;
    transition: opacity 0.15s;
    border-radius: 4px;
  }

  .lightbox-nav:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.2);
  }

  .lightbox-prev {
    left: 1rem;
  }

  .lightbox-next {
    right: 1rem;
  }

  .lightbox-img-wrapper {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .lightbox-img {
    max-width: 90vw;
    max-height: 85vh;
    object-fit: contain;
    border-radius: 4px;
  }

  .lightbox-info {
    position: absolute;
    bottom: 1rem;
    left: 50%;
    transform: translateX(-50%);
    color: #ccc;
    font-size: 0.875rem;
    display: flex;
    gap: 1rem;
    background: rgba(0, 0, 0, 0.6);
    padding: 0.5rem 1rem;
    border-radius: 4px;
  }

  .lightbox-counter {
    color: #888;
  }
</style>
