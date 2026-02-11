<script>
  import { onMount } from 'svelte';
  import { DEFAULT_CONFIG, TIMING, API } from '../constants.js';
  import { handleError } from '../errorHandler.js';
  import EyeIndicator from './visual/EyeIndicator.svelte';
  import AlignmentIndicator from './visual/AlignmentIndicator.svelte';
  import BrightnessIndicator from './visual/BrightnessIndicator.svelte';

  let { disabled = false } = $props();

  let isOpen = $state(false);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state(null);
  let saveMessage = $state(null);
  let activeTab = $state('face');

  // Config state - initialized from DEFAULT_CONFIG, then loaded from API
  let config = $state(JSON.parse(JSON.stringify(DEFAULT_CONFIG)));

  async function loadConfig() {
    loading = true;
    error = null;
    try {
      const res = await fetch(API.config);
      if (!res.ok) throw res;
      config = await res.json();
    } catch (e) {
      error = await handleError('Failed to load config', e);
    } finally {
      loading = false;
    }
  }

  async function saveConfig() {
    saving = true;
    error = null;
    saveMessage = null;
    try {
      // Ensure all numeric values are proper numbers (not strings from range inputs)
      const configToSend = {
        processing: {
          max_workers: Number(config.processing.max_workers),
          face_resolution: {
            enabled: config.processing.face_resolution.enabled,
            min_size: Number(config.processing.face_resolution.min_size),
          },
          brightness: {
            enabled: config.processing.brightness.enabled,
            min_brightness: Number(config.processing.brightness.min_brightness),
            max_brightness: Number(config.processing.brightness.max_brightness),
          },
          blur: {
            enabled: config.processing.blur.enabled,
            min_sharpness: Number(config.processing.blur.min_sharpness),
          },
          head_pose: {
            enabled: config.processing.head_pose.enabled,
            max_yaw: Number(config.processing.head_pose.max_yaw),
            max_pitch: Number(config.processing.head_pose.max_pitch),
            max_roll: Number(config.processing.head_pose.max_roll),
          },
          eye_filter: {
            enabled: config.processing.eye_filter.enabled,
            min_ear: Number(config.processing.eye_filter.min_ear),
          },
          output: {
            size: Math.max(64, Math.min(4096, Number(config.processing.output.size) || 512)),
            keep_intermediates: config.processing.output.keep_intermediates,
          },
          alignment: {
            eye_distance: Number(config.processing.alignment.eye_distance),
          },
          timestamp: {
            enabled: config.processing.timestamp.enabled,
            position: config.processing.timestamp.position,
            year: config.processing.timestamp.year,
            month: config.processing.timestamp.month,
            day: config.processing.timestamp.day,
          },
          time_interval: {
            enabled: config.processing.time_interval.enabled,
            max_photos: Number(config.processing.time_interval.max_photos),
            time_range: config.processing.time_interval.time_range,
          },
        },
        video: {
          enabled: config.video.enabled,
          framerate: Number(config.video.framerate),
          codec: config.video.codec,
          crf: Number(config.video.crf),
        },
      };

      const res = await fetch(API.config, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(configToSend),
      });
      if (!res.ok) throw res;
      config = await res.json();
      saveMessage = 'Settings saved';
      setTimeout(() => (saveMessage = null), TIMING.messageDisplayDuration);
    } catch (e) {
      error = await handleError('Failed to save config', e);
    } finally {
      saving = false;
    }
  }

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
              <!-- Face Resolution Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Face Resolution</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.face_resolution.enabled}
                  />
                </div>

                {#if config.processing.face_resolution.enabled}
                  <div class="setting-row sub-setting">
                    <label for="min-face-size">
                      <span class="setting-label">Min Face Size</span>
                      <span class="setting-hint">Minimum face resolution in pixels</span>
                    </label>
                    <div class="setting-control">
                      <input
                        id="min-face-size"
                        type="range"
                        bind:value={config.processing.face_resolution.min_size}
                        min="20"
                        max="200"
                        step="10"
                      />
                      <span class="value">{config.processing.face_resolution.min_size}px</span>
                    </div>
                  </div>
                {/if}
              </div>

              <!-- Brightness Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Brightness Filter</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.brightness.enabled}
                  />
                </div>

                {#if config.processing.brightness.enabled}
                  <div class="brightness-visual-control sub-setting">
                    <div class="brightness-hint">
                      Discard photos if the face is under/over exposed
                    </div>
                    <div class="brightness-indicator-container">
                      <BrightnessIndicator
                        bind:minBrightness={config.processing.brightness.min_brightness}
                        bind:maxBrightness={config.processing.brightness.max_brightness}
                      />
                    </div>
                  </div>
                {/if}
              </div>

              <!-- Blur Detection Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Blur Filter</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.blur.enabled}
                  />
                </div>

                {#if config.processing.blur.enabled}
                  <div class="setting-row sub-setting">
                    <label for="min-sharpness">
                      <span class="setting-label">Min Sharpness</span>
                      <span class="setting-hint">Discard blurry faces</span>
                    </label>
                    <div class="setting-control">
                      <input
                        id="min-sharpness"
                        type="range"
                        bind:value={config.processing.blur.min_sharpness}
                        min="10"
                        max="50"
                        step="1"
                      />
                      <span class="value">{Number(config.processing.blur.min_sharpness).toFixed(0)}</span>
                    </div>
                  </div>
                {/if}
              </div>

              <!-- Head Pose Filter Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Head Pose Filter</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.head_pose.enabled}
                  />
                </div>

                {#if config.processing.head_pose.enabled}
                  <div class="setting-row sub-setting">
                    <label for="max-yaw">
                      <span class="setting-label">Max Yaw</span>
                      <span class="setting-hint">Maximum left/right turn angle</span>
                    </label>
                    <div class="setting-control">
                      <input
                        id="max-yaw"
                        type="range"
                        bind:value={config.processing.head_pose.max_yaw}
                        min="5"
                        max="90"
                        step="5"
                      />
                      <span class="value">{config.processing.head_pose.max_yaw.toFixed(0)}°</span>
                    </div>
                  </div>

                  <div class="setting-row sub-setting">
                    <label for="max-pitch">
                      <span class="setting-label">Max Pitch</span>
                      <span class="setting-hint">Maximum up/down tilt angle</span>
                    </label>
                    <div class="setting-control">
                      <input
                        id="max-pitch"
                        type="range"
                        bind:value={config.processing.head_pose.max_pitch}
                        min="5"
                        max="90"
                        step="5"
                      />
                      <span class="value">{config.processing.head_pose.max_pitch.toFixed(0)}°</span>
                    </div>
                  </div>
                {/if}
              </div>

              <!-- Eye Filter Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Eye Filter (Blink Detection)</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.eye_filter.enabled}
                  />
                </div>

                {#if config.processing.eye_filter.enabled}
                  <div class="setting-row sub-setting">
                    <label for="min-ear">
                      <span class="setting-label">Minimum eye opening</span>
                      <span class="setting-hint">Discard image if Eye Aspect Ratio is bellow this value</span>
                    </label>
                    <div class="setting-control-with-visual">
                      <input
                        id="min-ear"
                        type="range"
                        bind:value={config.processing.eye_filter.min_ear}
                        min="0.1"
                        max="0.4"
                        step="0.02"
                      />
                      <span class="value">{config.processing.eye_filter.min_ear.toFixed(2)}</span>
                      <div class="inline-visual-indicator">
                        <EyeIndicator ear={config.processing.eye_filter.min_ear} />
                      </div>
                    </div>
                  </div>
                {/if}
              </div>

              <div class="setting-row checkbox-row">
                <label for="keep-intermediates">
                  <span class="setting-label">Keep Debug Images</span>
                  <span class="setting-hint">Save intermediate processing visualizations</span>
                </label>
                <input id="keep-intermediates" type="checkbox" bind:checked={config.processing.output.keep_intermediates} />
              </div>

            </fieldset>
          {:else if activeTab === 'output'}
            <fieldset disabled={disabled || saving}>
              <div class="setting-section">
                <div class="setting-row">
                  <label for="output-size">
                    <span class="setting-label">Output Size</span>
                    <span class="setting-hint">Final image dimensions (square)</span>
                  </label>
                  <div class="setting-control">
                    <input
                      id="output-size"
                      type="number"
                      class="inline-number"
                      bind:value={config.processing.output.size}
                      min="64"
                      max="4096"
                    />
                    <span class="setting-label">px</span>
                  </div>
                </div>

                <div class="setting-row">
                  <label for="max-workers">
                    <span class="setting-label">Parallel Workers</span>
                    <span class="setting-hint">Concurrent image processing tasks</span>
                  </label>
                  <div class="setting-control">
                    <input
                      id="max-workers"
                      type="range"
                      bind:value={config.processing.max_workers}
                      min="1"
                      max="16"
                      step="1"
                    />
                    <span class="value">{config.processing.max_workers}</span>
                  </div>
                </div>
              </div>

              <!-- Time Interval Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Time Interval</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.time_interval.enabled}
                  />
                </div>

                {#if config.processing.time_interval.enabled}
                  <div class="setting-row sub-setting">
                    <span class="setting-label">Keep at most</span>
                    <div class="setting-control">
                      <input
                        id="max-photos"
                        type="number"
                        class="inline-number"
                        bind:value={config.processing.time_interval.max_photos}
                        min="1"
                        max="10"
                      />
                      <span class="setting-label">per</span>
                      <div class="toggle-group">
                        {#each [['day', 'day'], ['week', 'week'], ['month', 'month']] as [value, label]}
                          <button
                            type="button"
                            class="toggle-btn-option"
                            class:active={config.processing.time_interval.time_range === value}
                            onclick={() => config.processing.time_interval.time_range = value}
                          >{label}</button>
                        {/each}
                      </div>
                    </div>
                  </div>
                {/if}
              </div>

              <!-- Timestamp Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Timestamp Overlay</span>
                  <input
                    type="checkbox"
                    bind:checked={config.processing.timestamp.enabled}
                  />
                </div>

                {#if config.processing.timestamp.enabled}
                  <div class="setting-row sub-setting">
                    <label for="timestamp-position">
                      <span class="setting-label">Position</span>
                      <span class="setting-hint">Where to place the timestamp</span>
                    </label>
                    <select id="timestamp-position" bind:value={config.processing.timestamp.position}>
                      <option value="top_left">Top Left</option>
                      <option value="top_right">Top Right</option>
                      <option value="bottom_left">Bottom Left</option>
                      <option value="bottom_right">Bottom Right</option>
                    </select>
                  </div>

                  <div class="setting-row sub-setting">
                    <span class="setting-label">Date Components</span>
                    <div class="toggle-group">
                      <button
                        type="button"
                        class="toggle-btn-option"
                        class:active={config.processing.timestamp.year}
                        onclick={() => config.processing.timestamp.year = !config.processing.timestamp.year}
                      >Year</button>
                      <button
                        type="button"
                        class="toggle-btn-option"
                        class:active={config.processing.timestamp.month}
                        onclick={() => config.processing.timestamp.month = !config.processing.timestamp.month}
                      >Month</button>
                      <button
                        type="button"
                        class="toggle-btn-option"
                        class:active={config.processing.timestamp.day}
                        onclick={() => config.processing.timestamp.day = !config.processing.timestamp.day}
                      >Day</button>
                    </div>
                  </div>
                {/if}
              </div>

              <!-- Alignment Section -->
              <div class="setting-section">
                <div class="section-header">
                  <span class="section-title">Final Image Composition</span>
                </div>
                <div class="setting-row sub-setting">
                  <label for="eye-distance">
                    <span class="setting-label">Eye Distance</span>
                    <span class="setting-hint">Distance between eyes as % of image width (larger = zoom in)</span>
                  </label>
                  <div class="setting-control-with-visual">
                    <input
                      id="eye-distance"
                      type="range"
                      bind:value={config.processing.alignment.eye_distance}
                      min="0.05"
                      max="0.4"
                      step="0.01"
                    />
                    <span class="value">{(config.processing.alignment.eye_distance * 100).toFixed(0)}%</span>
                    <div class="inline-visual-indicator">
                      <AlignmentIndicator eyeDistance={config.processing.alignment.eye_distance} />
                    </div>
                  </div>
                </div>
              </div>
            </fieldset>
          {:else if activeTab === 'video'}
            <fieldset disabled={disabled || saving}>
              <div class="setting-row">
                <label for="video-framerate">
                  <span class="setting-label">Framerate</span>
                  <span class="setting-hint">Video frames per second</span>
                </label>
                <div class="setting-control">
                  <input
                    id="video-framerate"
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
                <label for="video-crf">
                  <span class="setting-label">Quality (CRF)</span>
                  <span class="setting-hint">Lower = better quality, larger file</span>
                </label>
                <div class="setting-control">
                  <input
                    id="video-crf"
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
                <label for="video-codec">
                  <span class="setting-label">Codec</span>
                  <span class="setting-hint">Video encoding format</span>
                </label>
                <select id="video-codec" bind:value={config.video.codec}>
                  <option value="libx264">H.264 (libx264)</option>
                  <option value="libx265">H.265 (libx265)</option>
                  <option value="libvpx-vp9">VP9 (libvpx-vp9)</option>
                </select>
              </div>

              <div class="setting-row checkbox-row">
                <label for="video-enabled">
                  <span class="setting-label">Auto-compile Video</span>
                  <span class="setting-hint">Automatically create video after processing</span>
                </label>
                <input id="video-enabled" type="checkbox" bind:checked={config.video.enabled} />
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

  /* Settings section with header */
  .setting-section {
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #333;
  }

  .setting-section:last-child {
    margin-bottom: 0;
    padding-bottom: 0;
    border-bottom: none;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .section-title {
    font-size: 0.875rem;
    font-weight: 600;
    color: #e0e0e0;
  }

  .section-header input[type='checkbox'] {
    width: 1.25rem;
    height: 1.25rem;
    accent-color: #4f46e5;
    cursor: pointer;
  }

  .sub-setting {
    padding-left: 1.5rem;
    border-left: 2px solid #333;
    margin-left: 0.75rem;
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

  /* Visual indicator layout */
  .setting-control-with-visual {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .inline-visual-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0.5rem;
    background: #0f0f0f;
    border: 1px solid #333;
    border-radius: 6px;
    width: 96px;
    height: 96px;
  }

  .inline-number {
    width: 3rem;
    padding: 0.3rem 0.4rem;
    background: #0f0f0f;
    border: 1px solid #333;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 0.875rem;
    text-align: center;
    -moz-appearance: textfield;
  }

  .inline-number::-webkit-inner-spin-button,
  .inline-number::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .inline-number:focus {
    outline: none;
    border-color: #4f46e5;
  }

  /* Toggle button group */
  .toggle-group {
    display: flex;
    gap: 0;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid #333;
  }

  .toggle-btn-option {
    padding: 0.4rem 0.75rem;
    background: #0f0f0f;
    border: none;
    border-right: 1px solid #333;
    color: #888;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .toggle-btn-option:last-child {
    border-right: none;
  }

  .toggle-btn-option:hover {
    background: #252525;
    color: #e0e0e0;
  }

  .toggle-btn-option.active {
    background: #4f46e5;
    color: #fff;
    font-weight: 600;
  }

  /* Brightness visual control */
  .brightness-visual-control {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .brightness-hint {
    font-size: 0.75rem;
    color: #666;
    text-align: center;
  }

  .brightness-indicator-container {
    display: flex;
    justify-content: center;
    padding: 1rem;
    background: #0f0f0f;
    border: 1px solid #333;
    border-radius: 6px;
  }
</style>
