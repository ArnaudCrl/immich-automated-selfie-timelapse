<script>
  import { onMount, onDestroy } from 'svelte';
  import ConnectionStatus from './lib/components/ConnectionStatus.svelte';
  import GalleryView from './lib/components/GalleryView.svelte';
  import OutputManager from './lib/components/OutputManager.svelte';
  import PeopleSelector from './lib/components/PeopleSelector.svelte';
  import ProcessingControls from './lib/components/ProcessingControls.svelte';
  import ProgressDisplay from './lib/components/ProgressDisplay.svelte';
  import ResultsView from './lib/components/ResultsView.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';

  // Configuration constants
  const POLL_INTERVAL_MS = 500; // Progress polling interval during job execution
  const STORAGE_KEY_PERSON = 'immich-timelapse-selected-person';

  // Load persisted state from localStorage
  function loadPersistedPersonId() {
    try {
      const stored = localStorage.getItem(STORAGE_KEY_PERSON);
      if (stored) {
        const parsed = JSON.parse(stored);
        return parsed?.id || null;
      }
    } catch (e) {
      console.warn('Failed to load persisted person:', e);
    }
    return null;
  }

  function persistSelectedPerson(person) {
    try {
      if (person) {
        localStorage.setItem(STORAGE_KEY_PERSON, JSON.stringify({ id: person.id, name: person.name }));
      } else {
        localStorage.removeItem(STORAGE_KEY_PERSON);
      }
    } catch (e) {
      console.warn('Failed to persist person:', e);
    }
  }

  let connectionOk = $state(false);
  let selectedPerson = $state(null);
  let initialSelectedPersonId = $state(loadPersistedPersonId());
  let jobStatus = $state('idle');
  let progress = $state({ completed: 0, total: 0, message: '' });
  let pollInterval = $state(null);

  // View state management for gallery
  let currentView = $state('main'); // 'main' | 'gallery'
  let galleryFolder = $state(null);
  let savedScrollPosition = $state(0);

  // Output folder management
  let outputFolderRefreshKey = $state(0);
  let outputFolders = $state([]);

  let isJobRunning = $derived(
    jobStatus === 'running' || jobStatus === 'compiling_video' || jobStatus === 'cancelling'
  );

  // Match backend's sanitize_folder_name logic for video path construction
  function sanitizeFolderName(name, id) {
    const base = name && name.trim() ? name : id;
    let sanitized = '';
    for (const c of base) {
      if (/^[\p{L}\p{N}\-_ ]$/u.test(c)) {
        sanitized += c;
      } else {
        sanitized += '_';
      }
    }
    sanitized = sanitized.trim();
    return sanitized.length > 50 ? sanitized.slice(0, 50) : sanitized;
  }

  // Compute folder name from progress for video display
  let completedFolderName = $derived(
    progress.person_name || progress.person_id
      ? sanitizeFolderName(progress.person_name, progress.person_id)
      : null
  );

  function handleFoldersLoaded(folders) {
    outputFolders = folders;
  }

  function handleFolderDeleted(folderName) {
    // If the deleted folder matches the completed job's folder, reset to idle
    // folderName === null means all folders were deleted
    if (jobStatus === 'completed' && (folderName === null || folderName === completedFolderName)) {
      jobStatus = 'idle';
      progress = { completed: 0, total: 0, message: '' };
    }
  }

  function openGallery(folder) {
    // Save scroll position before switching to gallery
    savedScrollPosition = window.scrollY;
    galleryFolder = folder;
    currentView = 'gallery';
    // Scroll to top for gallery view
    window.scrollTo(0, 0);
  }

  function closeGallery() {
    galleryFolder = null;
    currentView = 'main';
    // Check for any running job (e.g., video compilation started from gallery)
    checkAndPollProgress();
    // Restore scroll position after DOM updates
    requestAnimationFrame(() => {
      window.scrollTo(0, savedScrollPosition);
    });
  }

  function handleConnectionChange(data) {
    connectionOk = data.connected;
    // Check for running job when connection is established
    if (data.connected) {
      checkAndPollProgress();
    }
  }

  function handlePersonSelect(person) {
    selectedPerson = person;
    persistSelectedPerson(person);
  }

  function handleJobUpdate(data) {
    jobStatus = data.status;
    progress = data;

    // Start polling if job just started
    if (data.status === 'running' || data.status === 'compiling_video') {
      startPolling();
    }
  }

  async function checkAndPollProgress() {
    try {
      const res = await fetch('/api/progress');
      const data = await res.json();

      // Only restore status if a job is actively running, or if we're not in idle state.
      // This prevents restoring 'completed' status after we've manually dismissed it.
      const isActiveJob = data.status === 'running' || data.status === 'compiling_video' || data.status === 'cancelling';
      if (isActiveJob || jobStatus !== 'idle') {
        jobStatus = data.status;
        progress = data;
      }

      // If a job is running, start polling
      if (isActiveJob) {
        startPolling();
      }
    } catch (e) {
      console.error('Failed to check progress:', e);
    }
  }

  async function pollProgress() {
    try {
      const res = await fetch('/api/progress');
      const data = await res.json();

      const previousStatus = jobStatus;
      jobStatus = data.status;
      progress = data;

      // Stop polling when job completes
      if (data.status === 'completed' || data.status === 'cancelled' || data.status === 'error' || data.status === 'idle') {
        stopPolling();
        // Refresh output folders when job finishes (was running before)
        if (previousStatus === 'running' || previousStatus === 'compiling_video' || previousStatus === 'cancelling') {
          outputFolderRefreshKey++;
        }
      }
    } catch (e) {
      console.error('Poll failed:', e);
    }
  }

  function startPolling() {
    if (pollInterval) return; // Already polling
    pollInterval = setInterval(pollProgress, POLL_INTERVAL_MS);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  onMount(() => {
    // Initial check will happen when connection is established
  });

  onDestroy(() => {
    stopPolling();
  });
</script>

<main>
  <header>
    <h1>Immich Selfie Timelapse</h1>
    <ConnectionStatus onchange={handleConnectionChange} />
  </header>

  {#if connectionOk}
    {#if currentView === 'gallery' && galleryFolder}
      <section class="gallery">
        <GalleryView
          folderName={galleryFolder.name}
          onBack={closeGallery}
          disabled={isJobRunning}
        />
      </section>
    {:else}
      <section class="settings">
        <SettingsPanel disabled={isJobRunning} />
      </section>

      <section class="controls">
        <PeopleSelector
          onselect={handlePersonSelect}
          disabled={isJobRunning}
          initialSelectedId={initialSelectedPersonId}
        />

        {#if selectedPerson && !isJobRunning}
          <ProcessingControls
            personId={selectedPerson.id}
            personName={selectedPerson.name}
            {jobStatus}
            {outputFolders}
            onupdate={handleJobUpdate}
          />
        {/if}
      </section>

      {#if jobStatus !== 'idle'}
        <section class="progress">
          <ProgressDisplay {jobStatus} {progress} />
        </section>
      {/if}

      {#if jobStatus === 'completed'}
        <section class="results">
          <ResultsView folderName={completedFolderName} />
        </section>
      {/if}

      <section class="output">
        <OutputManager
          disabled={isJobRunning}
          onOpenGallery={openGallery}
          refreshKey={outputFolderRefreshKey}
          onFoldersLoaded={handleFoldersLoaded}
          onFolderDeleted={handleFolderDeleted}
        />
      </section>
    {/if}
  {:else}
    <section class="not-connected">
      <p>Connect to your Immich server to get started.</p>
      <p class="hint">Make sure the backend is running and configured with your Immich API credentials.</p>
    </section>
  {/if}
</main>

<style>
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
    background: #0f0f0f;
    color: #e0e0e0;
    line-height: 1.6;
  }

  main {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #333;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    color: #fff;
  }

  section {
    margin-bottom: 2rem;
  }

  .not-connected {
    text-align: center;
    padding: 3rem;
    background: #1a1a1a;
    border-radius: 8px;
  }

  .not-connected p {
    margin-bottom: 0.5rem;
  }

  .hint {
    font-size: 0.875rem;
    color: #888;
  }
</style>
