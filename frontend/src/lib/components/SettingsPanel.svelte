<script>
  import { onMount } from 'svelte';

  let { disabled = false } = $props();

  let isOpen = $state(false);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state(null);
  let saveMessage = $state(null);

  // Config state
  let config = $state({
    processing: {
      resize_size: 512,
      face_resolution_threshold: 80,
      pose_threshold: 25.0,
      ear_threshold: 0.2,
      max_workers: 4,
      keep_intermediates: false,
    },
    video: {
      framerate: 15,
      enabled: true,
      codec: 'libx264',
      crf: 23,
    },
  });

  async function loadConfig() {
    loading = true;
    error = null;
    try {
      const res = await fetch('/api/config');
      if (!res.ok) throw new Error('Failed to load config');
      config = await res.json();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function saveConfig() {
    saving = true;
    error = null;
    saveMessage = null;
    try {
      const res = await fetch('/api/config', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || 'Failed to save config');
      }
      config = await res.json();
      saveMessage = 'Settings saved';
      setTimeout(() => (saveMessage = null), 2000);
    } catch (e) {
      error = e.message;
    } finally {
      saving = false;
    }
  }

  function toggle() {
    isOpen = !isOpen;
    if (isOpen && loading) {
      loadConfig();
    }
  }

  onMount(() => {
    loadConfig();
  });
</script>

<div class="settings-panel">
  <button class="toggle-btn" onclick={toggle}>
    <span class="icon">{isOpen ? '▼' : '▶'}</span>
    Settings
  </button>

  {#if isOpen}
    <div class="settings-content">
      {#if loading}
        <p class="loading">Loading settings...</p>
      {:else if error}
        <p class="error">{error}</p>
        <button class="retry-btn" onclick={loadConfig}>Retry</button>
      {:else}
        <div class="settings-grid">
          <fieldset disabled={disabled || saving}>
            <legend>Processing</legend>

            <label>
              <span>Output Size (px)</span>
              <input
                type="number"
                bind:value={config.processing.resize_size}
                min="128"
                max="2048"
                step="64"
              />
            </label>

            <label>
              <span>Min Face Size (px)</span>
              <input
                type="number"
                bind:value={config.processing.face_resolution_threshold}
                min="20"
                max="500"
                step="10"
              />
            </label>

            <label>
              <span>Pose Threshold (deg)</span>
              <input
                type="number"
                bind:value={config.processing.pose_threshold}
                min="5"
                max="90"
                step="5"
              />
            </label>

            <label>
              <span>Eye Aspect Ratio</span>
              <input
                type="number"
                bind:value={config.processing.ear_threshold}
                min="0.1"
                max="0.5"
                step="0.05"
              />
            </label>

            <label>
              <span>Parallel Workers</span>
              <input
                type="number"
                bind:value={config.processing.max_workers}
                min="1"
                max="32"
                step="1"
              />
            </label>

            <label class="checkbox-label">
              <input type="checkbox" bind:checked={config.processing.keep_intermediates} />
              <span>Keep debug images</span>
            </label>
          </fieldset>

          <fieldset disabled={disabled || saving}>
            <legend>Video</legend>

            <label>
              <span>Framerate (fps)</span>
              <input
                type="number"
                bind:value={config.video.framerate}
                min="1"
                max="60"
                step="1"
              />
            </label>

            <label>
              <span>Quality (CRF)</span>
              <input
                type="number"
                bind:value={config.video.crf}
                min="0"
                max="51"
                step="1"
              />
              <span class="hint">Lower = better quality, larger file</span>
            </label>

            <label>
              <span>Codec</span>
              <select bind:value={config.video.codec}>
                <option value="libx264">H.264 (libx264)</option>
                <option value="libx265">H.265 (libx265)</option>
                <option value="libvpx-vp9">VP9 (libvpx-vp9)</option>
              </select>
            </label>

            <label class="checkbox-label">
              <input type="checkbox" bind:checked={config.video.enabled} />
              <span>Auto-compile video</span>
            </label>
          </fieldset>
        </div>

        <div class="actions">
          <button class="save-btn" onclick={saveConfig} disabled={disabled || saving}>
            {saving ? 'Saving...' : 'Save Settings'}
          </button>
          {#if saveMessage}
            <span class="save-message">{saveMessage}</span>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .settings-panel {
    background: #1a1a1a;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .toggle-btn {
    width: 100%;
    padding: 1rem 1.5rem;
    background: none;
    border: none;
    color: #e0e0e0;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    text-align: left;
  }

  .toggle-btn:hover {
    background: #252525;
  }

  .icon {
    font-size: 0.75rem;
    color: #888;
  }

  .settings-content {
    padding: 0 1.5rem 1.5rem;
  }

  .loading,
  .error {
    font-size: 0.875rem;
    padding: 1rem;
    text-align: center;
  }

  .error {
    color: #dc2626;
  }

  .retry-btn {
    display: block;
    margin: 0 auto;
    padding: 0.5rem 1rem;
    background: #333;
    border: none;
    border-radius: 4px;
    color: #e0e0e0;
    cursor: pointer;
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1.5rem;
  }

  fieldset {
    border: 1px solid #333;
    border-radius: 6px;
    padding: 1rem;
  }

  fieldset:disabled {
    opacity: 0.5;
  }

  legend {
    font-size: 0.75rem;
    font-weight: 600;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0 0.5rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.75rem;
  }

  label > span:first-child {
    font-size: 0.75rem;
    color: #888;
  }

  input[type='number'],
  select {
    padding: 0.5rem 0.75rem;
    border: 1px solid #333;
    border-radius: 4px;
    background: #0f0f0f;
    color: #e0e0e0;
    font-size: 0.875rem;
  }

  input[type='number']:focus,
  select:focus {
    outline: none;
    border-color: #4f46e5;
  }

  select {
    cursor: pointer;
  }

  .checkbox-label {
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;
  }

  .checkbox-label input {
    width: 1rem;
    height: 1rem;
    accent-color: #4f46e5;
  }

  .checkbox-label span {
    font-size: 0.875rem;
    color: #e0e0e0;
  }

  .hint {
    font-size: 0.7rem;
    color: #666;
  }

  .actions {
    margin-top: 1.5rem;
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .save-btn {
    padding: 0.75rem 1.5rem;
    background: #4f46e5;
    border: none;
    border-radius: 6px;
    color: #fff;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .save-btn:hover:not(:disabled) {
    background: #4338ca;
  }

  .save-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .save-message {
    font-size: 0.875rem;
    color: #22c55e;
  }
</style>
