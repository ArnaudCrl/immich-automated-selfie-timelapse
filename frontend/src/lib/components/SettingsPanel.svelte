<script>
  import { onMount } from 'svelte';

  let { disabled = false } = $props();

  let isOpen = $state(false);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state(null);
  let saveMessage = $state(null);
  let activeTab = $state('face');

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

  // Default configuration values (must match backend defaults in config.rs)
  const DEFAULT_CONFIG = {
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
  };

  function resetToDefaults() {
    config = JSON.parse(JSON.stringify(DEFAULT_CONFIG));
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
  <button type="button" class="toggle-btn" onclick={toggle}>
    <span class="icon">{isOpen ? '▼' : '▶'}</span>
    Settings
  </button>

  {#if isOpen}
    <div class="settings-content">
      {#if loading}
        <p class="loading">Loading settings...</p>
      {:else if error}
        <p class="error">{error}</p>
        <button type="button" class="retry-btn" onclick={loadConfig}>Retry</button>
      {:else}
        <!-- Tab navigation -->
        <div class="tabs">
          <button
            type="button"
            class="tab"
            class:active={activeTab === 'face'}
            onclick={() => (activeTab = 'face')}
          >
            Face
          </button>
          <button
            type="button"
            class="tab"
            class:active={activeTab === 'output'}
            onclick={() => (activeTab = 'output')}
          >
            Output
          </button>
          <button
            type="button"
            class="tab"
            class:active={activeTab === 'video'}
            onclick={() => (activeTab = 'video')}
          >
            Video
          </button>
        </div>

        <!-- Tab content -->
        <div class="tab-content">
          {#if activeTab === 'face'}
            <fieldset disabled={disabled || saving}>
              <div class="setting-row">
                <label>
                  <span class="setting-label">Min Face Size</span>
                  <span class="setting-hint">Minimum face resolution in pixels</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.processing.face_resolution_threshold}
                    min="20"
                    max="200"
                    step="10"
                  />
                  <span class="value">{config.processing.face_resolution_threshold}px</span>
                </div>
              </div>

              <div class="setting-row">
                <label>
                  <span class="setting-label">Eye Aspect Ratio</span>
                  <span class="setting-hint">Threshold for eye openness detection</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.processing.ear_threshold}
                    min="0.1"
                    max="0.5"
                    step="0.05"
                  />
                  <span class="value">{config.processing.ear_threshold.toFixed(2)}</span>
                </div>
              </div>

              <div class="setting-row">
                <label>
                  <span class="setting-label">Max Head Rotation</span>
                  <span class="setting-hint">Maximum yaw angle in degrees</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.processing.pose_threshold}
                    min="5"
                    max="90"
                    step="5"
                  />
                  <span class="value">{config.processing.pose_threshold}°</span>
                </div>
              </div>
            </fieldset>
          {:else if activeTab === 'output'}
            <fieldset disabled={disabled || saving}>
              <div class="setting-row">
                <label>
                  <span class="setting-label">Output Size</span>
                  <span class="setting-hint">Final image dimensions (square)</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.processing.resize_size}
                    min="128"
                    max="1024"
                    step="64"
                  />
                  <span class="value">{config.processing.resize_size}px</span>
                </div>
              </div>

              <div class="setting-row">
                <label>
                  <span class="setting-label">Parallel Workers</span>
                  <span class="setting-hint">Concurrent image processing tasks</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.processing.max_workers}
                    min="1"
                    max="16"
                    step="1"
                  />
                  <span class="value">{config.processing.max_workers}</span>
                </div>
              </div>

              <div class="setting-row checkbox-row">
                <label>
                  <span class="setting-label">Keep Debug Images</span>
                  <span class="setting-hint">Save intermediate processing visualizations</span>
                </label>
                <input type="checkbox" bind:checked={config.processing.keep_intermediates} />
              </div>
            </fieldset>
          {:else if activeTab === 'video'}
            <fieldset disabled={disabled || saving}>
              <div class="setting-row">
                <label>
                  <span class="setting-label">Framerate</span>
                  <span class="setting-hint">Video frames per second</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.video.framerate}
                    min="1"
                    max="60"
                    step="1"
                  />
                  <span class="value">{config.video.framerate} fps</span>
                </div>
              </div>

              <div class="setting-row">
                <label>
                  <span class="setting-label">Quality (CRF)</span>
                  <span class="setting-hint">Lower = better quality, larger file</span>
                </label>
                <div class="setting-control">
                  <input
                    type="range"
                    bind:value={config.video.crf}
                    min="15"
                    max="40"
                    step="1"
                  />
                  <span class="value">{config.video.crf}</span>
                </div>
              </div>

              <div class="setting-row">
                <label>
                  <span class="setting-label">Codec</span>
                  <span class="setting-hint">Video encoding format</span>
                </label>
                <select bind:value={config.video.codec}>
                  <option value="libx264">H.264 (libx264)</option>
                  <option value="libx265">H.265 (libx265)</option>
                  <option value="libvpx-vp9">VP9 (libvpx-vp9)</option>
                </select>
              </div>

              <div class="setting-row checkbox-row">
                <label>
                  <span class="setting-label">Auto-compile Video</span>
                  <span class="setting-hint">Automatically create video after processing</span>
                </label>
                <input type="checkbox" bind:checked={config.video.enabled} />
              </div>
            </fieldset>
          {/if}
        </div>

        <div class="actions">
          <button type="button" class="save-btn" onclick={saveConfig} disabled={disabled || saving}>
            {saving ? 'Saving...' : 'Save Settings'}
          </button>
          <button type="button" class="reset-btn" onclick={resetToDefaults} disabled={disabled || saving}>
            Reset to Defaults
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

  /* Tabs */
  .tabs {
    display: flex;
    gap: 0.25rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid #333;
    padding-bottom: 0.5rem;
  }

  .tab {
    padding: 0.5rem 1rem;
    background: none;
    border: none;
    border-radius: 4px 4px 0 0;
    color: #888;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .tab:hover {
    color: #e0e0e0;
    background: #252525;
  }

  .tab.active {
    color: #e0e0e0;
    background: #333;
    font-weight: 600;
  }

  /* Tab content */
  .tab-content {
    min-height: 200px;
  }

  fieldset {
    border: none;
    padding: 0;
  }

  fieldset:disabled {
    opacity: 0.5;
  }

  .setting-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0;
    border-bottom: 1px solid #252525;
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-row label {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .setting-label {
    font-size: 0.875rem;
    color: #e0e0e0;
  }

  .setting-hint {
    font-size: 0.75rem;
    color: #666;
  }

  .setting-control {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  input[type='range'] {
    width: 120px;
    accent-color: #4f46e5;
  }

  .value {
    font-size: 0.875rem;
    color: #4f46e5;
    font-weight: 600;
    min-width: 60px;
    text-align: right;
  }

  select {
    padding: 0.5rem 0.75rem;
    border: 1px solid #333;
    border-radius: 4px;
    background: #0f0f0f;
    color: #e0e0e0;
    font-size: 0.875rem;
    cursor: pointer;
  }

  select:focus {
    outline: none;
    border-color: #4f46e5;
  }

  .checkbox-row {
    justify-content: space-between;
  }

  .checkbox-row input[type='checkbox'] {
    width: 1.25rem;
    height: 1.25rem;
    accent-color: #4f46e5;
    cursor: pointer;
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

  .reset-btn {
    padding: 0.75rem 1.5rem;
    background: #333;
    border: none;
    border-radius: 6px;
    color: #e0e0e0;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .reset-btn:hover:not(:disabled) {
    background: #444;
  }

  .reset-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .save-message {
    font-size: 0.875rem;
    color: #22c55e;
  }
</style>
