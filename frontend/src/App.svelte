<script>
  import { onDestroy } from 'svelte';
  import AlbumSelector from './lib/components/AlbumSelector.svelte';
  import ConnectionStatus from './lib/components/ConnectionStatus.svelte';
  import GalleryView from './lib/components/GalleryView.svelte';
  import OutputManager from './lib/components/OutputManager.svelte';
  import PeopleSelector from './lib/components/PeopleSelector.svelte';
  import ProcessingControls from './lib/components/ProcessingControls.svelte';
  import ProgressDisplay from './lib/components/ProgressDisplay.svelte';
  import ResultsView from './lib/components/ResultsView.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import { sanitizeFolderName } from './lib/utils.js';
  import { STORAGE_KEYS, WS, JOB_STATUS, API } from './lib/constants.js';

  // Load persisted state from localStorage
  function loadPersistedPersonId() {
    try {
      const stored = localStorage.getItem(STORAGE_KEYS.selectedPerson);
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
        localStorage.setItem(STORAGE_KEYS.selectedPerson, JSON.stringify({ id: person.id, name: person.name }));
      } else {
        localStorage.removeItem(STORAGE_KEYS.selectedPerson);
      }
    } catch (e) {
      console.warn('Failed to persist person:', e);
    }
  }

  function loadPersistedAlbumIds() {
    try {
      const stored = localStorage.getItem(STORAGE_KEYS.selectedAlbums);
      if (stored) {
        const parsed = JSON.parse(stored);
        return Array.isArray(parsed) ? parsed.map(a => a.id).filter(Boolean) : [];
      }
    } catch (e) {
      console.warn('Failed to load persisted albums:', e);
    }
    return [];
  }

  function persistSelectedAlbums(albums) {
    try {
      if (albums && albums.length > 0) {
        localStorage.setItem(STORAGE_KEYS.selectedAlbums, JSON.stringify(albums.map(a => ({ id: a.id, name: a.name }))));
      } else {
        localStorage.removeItem(STORAGE_KEYS.selectedAlbums);
      }
    } catch (e) {
      console.warn('Failed to persist albums:', e);
    }
  }

  let connectionOk = $state(false);
  let selectedPerson = $state(null);
  let initialSelectedPersonId = $state(loadPersistedPersonId());
  let selectedAlbums = $state([]);
  let initialSelectedAlbumIds = $state(loadPersistedAlbumIds());
  let jobStatus = $state('idle');
  let progress = $state({ completed: 0, total: 0, message: '' });
  let ws = null;
  let wsReconnectTimeout = null;

  // View state management for gallery
  let currentView = $state('main'); // 'main' | 'gallery'
  let galleryFolder = $state(null);
  let savedScrollPosition = $state(0);

  // Output folder management
  let outputFolders = $state([]);

  let isJobRunning = $derived(
    jobStatus === JOB_STATUS.running || jobStatus === JOB_STATUS.compiling || jobStatus === JOB_STATUS.cancelling
  );

  // Compute folder name from progress for video display
  let completedFolderName = $derived(
    progress.person_name || progress.person_id
      ? sanitizeFolderName(progress.person_name, progress.person_id)
      : null
  );

  async function loadOutputFolders() {
    try {
      const res = await fetch(API.output);
      if (res.ok) {
        outputFolders = await res.json();
      }
    } catch (e) {
      console.error('Failed to load output folders:', e);
    }
  }

  function handleFolderDeleted(folderName) {
    // If the deleted folder matches the completed job's folder, reset to idle
    // folderName === null means all folders were deleted
    if (jobStatus === JOB_STATUS.completed && (folderName === null || folderName === completedFolderName)) {
      jobStatus = JOB_STATUS.idle;
      progress = { completed: 0, total: 0, message: '' };
    }
    // Reload folder list after deletion
    loadOutputFolders();
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
    connectWebSocket();
    // Restore scroll position after DOM updates
    requestAnimationFrame(() => {
      window.scrollTo(0, savedScrollPosition);
    });
  }

  function handleConnectionChange(data) {
    connectionOk = data.connected;
    // Check for running job when connection is established
    if (data.connected) {
      connectWebSocket();
    }
  }

  function handlePersonSelect(person) {
    selectedPerson = person;
    persistSelectedPerson(person);
  }

  function handleAlbumSelect(albums) {
    selectedAlbums = albums;
    persistSelectedAlbums(albums);
  }

  function handleJobUpdate(data) {
    jobStatus = data.status;
    progress = data;
  }

  function connectWebSocket() {
    if (ws && (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN)) {
      return; // Already connected or connecting
    }

    ws = new WebSocket(WS.getUrl());

    ws.onopen = () => {
      console.log('WebSocket connected');
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        const previousStatus = jobStatus;

        // Only restore status if a job is actively running, or if we're not in idle state
        const isActiveJob = data.status === JOB_STATUS.running || data.status === JOB_STATUS.compiling || data.status === JOB_STATUS.cancelling;
        if (isActiveJob || jobStatus !== JOB_STATUS.idle) {
          jobStatus = data.status;
          progress = data;
        }

        // Refresh output folders when job finishes (was running before)
        if (data.status === JOB_STATUS.completed || data.status === JOB_STATUS.cancelled || data.status === JOB_STATUS.error) {
          if (previousStatus === JOB_STATUS.running || previousStatus === JOB_STATUS.compiling || previousStatus === JOB_STATUS.cancelling) {
            loadOutputFolders();
          }
        }
      } catch (e) {
        console.error('Failed to parse WebSocket message:', e);
      }
    };

    ws.onclose = () => {
      console.log('WebSocket disconnected');
      ws = null;
      // Reconnect if connection was established (connectionOk is true)
      if (connectionOk) {
        scheduleReconnect();
      }
    };

    ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }

  function scheduleReconnect() {
    if (wsReconnectTimeout) return; // Already scheduled
    wsReconnectTimeout = setTimeout(() => {
      wsReconnectTimeout = null;
      if (connectionOk) {
        connectWebSocket();
      }
    }, WS.reconnectDelay);
  }

  function disconnectWebSocket() {
    if (wsReconnectTimeout) {
      clearTimeout(wsReconnectTimeout);
      wsReconnectTimeout = null;
    }
    if (ws) {
      ws.close();
      ws = null;
    }
  }

  // Reload folders when connection is established
  $effect(() => {
    if (connectionOk) {
      loadOutputFolders();
    }
  });

  onDestroy(() => {
    disconnectWebSocket();
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

        {#if selectedPerson}
          <AlbumSelector
            onselect={handleAlbumSelect}
            disabled={isJobRunning}
            initialSelectedIds={initialSelectedAlbumIds}
          />
        {/if}

        {#if selectedPerson && !isJobRunning}
          <ProcessingControls
            personId={selectedPerson.id}
            personName={selectedPerson.name}
            albums={selectedAlbums}
            {jobStatus}
            {outputFolders}
            onupdate={handleJobUpdate}
          />
        {/if}
      </section>

      {#if jobStatus !== JOB_STATUS.idle}
        <section class="progress">
          <ProgressDisplay {jobStatus} {progress} />
        </section>
      {/if}

      {#if jobStatus === JOB_STATUS.completed}
        <section class="results">
          <ResultsView folderName={completedFolderName} />
        </section>
      {/if}

      <section class="output">
        <OutputManager
          disabled={isJobRunning}
          folders={outputFolders}
          onOpenGallery={openGallery}
          onFolderDeleted={handleFolderDeleted}
        />
      </section>
    {/if}
  {:else}
    <section class="not-connected">
      <p>Connect to your Immich server to get started.</p>
      <p class="hint">Make sure the backend is running and configured with your Immich API credentials.</p>
      <p class="hint">
        You need an Immich API key with the following permissions:
        <code>asset.download</code>, <code>asset.read</code>, <code>asset.view</code>, <code>person.read</code>, <code>album.read</code>, <code>server.about</code>.
        <br />
        See the <a href="https://immich.app/docs/features/command-line-interface#obtain-the-api-key" target="_blank" rel="noopener noreferrer">Immich docs</a> to create one.
      </p>
    </section>
  {/if}
</main>

<style>
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
    margin-bottom: 1rem;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 1rem;
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
