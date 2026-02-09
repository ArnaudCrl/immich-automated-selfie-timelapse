<script>
  import { onMount } from 'svelte';
  import { handleError } from '../errorHandler.js';
  import { API } from '../constants.js';

  let { onchange } = $props();

  let status = $state('checking');
  let version = $state(null);
  let error = $state(null);

  async function checkConnection() {
    status = 'checking';
    error = null;

    try {
      const res = await fetch(API.connection);
      const data = await res.json();

      if (data.connected) {
        status = 'connected';
        version = data.version;
        onchange?.({ connected: true, version: data.version });
      } else {
        status = 'error';
        error = data.error || 'Connection failed';
        onchange?.({ connected: false });
      }
    } catch (e) {
      status = 'error';
      error = await handleError('Cannot reach backend server', e);
      onchange?.({ connected: false });
    }
  }

  onMount(() => {
    checkConnection();
  });
</script>

<div class="connection-status" class:connected={status === 'connected'} class:error={status === 'error'}>
  {#if status === 'checking'}
    <span class="indicator checking"></span>
    <span>Connecting...</span>
  {:else if status === 'connected'}
    <span class="indicator connected"></span>
    <span>Immich {version}</span>
  {:else}
    <span class="indicator error"></span>
    <button type="button" onclick={checkConnection} class="retry">
      {error} - Retry
    </button>
  {/if}
</div>

<style>
  .connection-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    padding: 0.5rem 1rem;
    border-radius: 9999px;
    background: #1a1a1a;
  }

  .indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .indicator.checking {
    background: #f59e0b;
    animation: pulse 1s infinite;
  }

  .indicator.connected {
    background: #22c55e;
  }

  .indicator.error {
    background: #ef4444;
  }

  .retry {
    background: none;
    border: none;
    color: #ef4444;
    cursor: pointer;
    font-size: inherit;
  }

  .retry:hover {
    text-decoration: underline;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
