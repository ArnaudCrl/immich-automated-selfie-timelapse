<script>
  import ConnectionStatus from './lib/components/ConnectionStatus.svelte';
  import OutputManager from './lib/components/OutputManager.svelte';
  import PeopleSelector from './lib/components/PeopleSelector.svelte';
  import ProcessingControls from './lib/components/ProcessingControls.svelte';
  import ProgressDisplay from './lib/components/ProgressDisplay.svelte';
  import ResultsView from './lib/components/ResultsView.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';

  let connectionOk = $state(false);
  let selectedPerson = $state(null);
  let jobStatus = $state('idle');
  let progress = $state({ completed: 0, total: 0, message: '' });

  let isJobRunning = $derived(
    jobStatus === 'running' || jobStatus === 'compiling_video' || jobStatus === 'cancelling'
  );

  function handleConnectionChange(data) {
    connectionOk = data.connected;
  }

  function handlePersonSelect(person) {
    selectedPerson = person;
  }

  function handleJobUpdate(data) {
    jobStatus = data.status;
    progress = data;
  }
</script>

<main>
  <header>
    <h1>Immich Selfie Timelapse</h1>
    <ConnectionStatus onchange={handleConnectionChange} />
  </header>

  {#if connectionOk}
    <section class="settings">
      <SettingsPanel disabled={isJobRunning} />
    </section>

    <section class="controls">
      <PeopleSelector
        onselect={handlePersonSelect}
        disabled={isJobRunning}
      />

      {#if selectedPerson}
        <ProcessingControls
          personId={selectedPerson.id}
          personName={selectedPerson.name}
          {jobStatus}
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
        <ResultsView />
      </section>
    {/if}

    <section class="output">
      <OutputManager disabled={isJobRunning} />
    </section>
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
