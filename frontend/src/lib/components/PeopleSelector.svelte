<script>
  import { onMount } from 'svelte';

  let { disabled = false, onselect } = $props();

  let people = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let searchQuery = $state('');
  let selectedId = $state(null);

  let filteredPeople = $derived(
    people.filter(p => {
      const name = p.name || 'Unnamed';
      return name.toLowerCase().includes(searchQuery.toLowerCase());
    })
  );

  async function loadPeople() {
    loading = true;
    error = null;

    try {
      const res = await fetch('/api/people');
      if (!res.ok) throw new Error('Failed to load people');
      people = await res.json();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  function selectPerson(person) {
    if (disabled) return;
    selectedId = person.id;
    onselect?.(person);
  }

  onMount(() => {
    loadPeople();
  });
</script>

<div class="people-selector">
  <h2>Select Person</h2>

  {#if loading}
    <div class="loading">Loading people...</div>
  {:else if error}
    <div class="error">
      {error}
      <button onclick={loadPeople}>Retry</button>
    </div>
  {:else}
    <input
      type="text"
      placeholder="Search people..."
      bind:value={searchQuery}
      {disabled}
      class="search"
    />

    <div class="people-grid">
      {#each filteredPeople as person (person.id)}
        <button
          class="person-card"
          class:selected={selectedId === person.id}
          onclick={() => selectPerson(person)}
          {disabled}
        >
          <div class="avatar">
            <img
              src="/api/people/{person.id}/thumbnail"
              alt={person.name || 'Person'}
              onerror={(e) => e.target.style.display = 'none'}
            />
            <span class="fallback">{(person.name || 'U').charAt(0).toUpperCase()}</span>
          </div>
          <span class="name">{person.name || 'Unnamed'}</span>
        </button>
      {:else}
        <p class="no-results">No people found</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .people-selector {
    background: #1a1a1a;
    border-radius: 8px;
    padding: 1.5rem;
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 1rem;
    color: #fff;
  }

  .search {
    width: 100%;
    padding: 0.75rem 1rem;
    border: 1px solid #333;
    border-radius: 6px;
    background: #0f0f0f;
    color: #e0e0e0;
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .search:focus {
    outline: none;
    border-color: #4f46e5;
  }

  .search:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .people-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 0.75rem;
    max-height: 300px;
    overflow-y: auto;
  }

  .person-card {
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
  }

  .person-card:hover:not(:disabled) {
    background: #303030;
  }

  .person-card.selected {
    border-color: #4f46e5;
    background: #2a2a4a;
  }

  .person-card:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .avatar {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: linear-gradient(135deg, #4f46e5, #7c3aed);
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    font-size: 1.25rem;
    color: #fff;
    overflow: hidden;
    position: relative;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    position: absolute;
    top: 0;
    left: 0;
    z-index: 1;
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

  .loading, .error, .no-results {
    text-align: center;
    padding: 2rem;
    color: #888;
  }

  .error {
    color: #ef4444;
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
