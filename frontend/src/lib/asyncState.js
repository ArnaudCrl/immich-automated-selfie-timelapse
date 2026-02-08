/**
 * Async operation state management utilities.
 * Helps manage loading, error, and success states for async operations.
 */

/**
 * Create an async state manager using Svelte 5 runes
 * @returns {Object} State manager with loading, error, and execute methods
 *
 * @example
 * const asyncOp = createAsyncState();
 * await asyncOp.execute(async () => {
 *   const res = await fetch('/api/data');
 *   return await res.json();
 * });
 * // Use asyncOp.loading, asyncOp.error in template
 */
export function createAsyncState() {
  let loading = $state(false);
  let error = $state(null);

  async function execute(operation, options = {}) {
    const { onSuccess, onError } = options;

    loading = true;
    error = null;

    try {
      const result = await operation();
      onSuccess?.(result);
      return { success: true, data: result };
    } catch (err) {
      error = err.message || String(err);
      onError?.(error);
      return { success: false, error };
    } finally {
      loading = false;
    }
  }

  function reset() {
    loading = false;
    error = null;
  }

  return {
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    execute,
    reset,
  };
}

/**
 * Get button text based on loading state
 * @param {boolean} loading - Whether operation is in progress
 * @param {string} defaultText - Text when not loading
 * @param {string} loadingText - Text when loading (defaults to defaultText + "...")
 * @returns {string} Button text
 */
export function getButtonText(loading, defaultText, loadingText = null) {
  if (loading) {
    return loadingText || `${defaultText}...`;
  }
  return defaultText;
}

/**
 * Create a standardized async operation handler with loading state
 * Useful for button click handlers
 *
 * @param {Function} operation - Async operation to execute
 * @param {Object} stateRefs - Object with loading state ref (for mutation)
 * @param {Object} options - Additional options
 * @returns {Function} Handler function
 *
 * @example
 * let saving = $state(false);
 * const handleSave = createAsyncHandler(
 *   async () => { await saveData(); },
 *   { loading: () => saving, setLoading: (val) => { saving = val; } }
 * );
 */
export function createAsyncHandler(operation, stateRefs, options = {}) {
  const { onSuccess, onError, confirmMessage } = options;

  return async function handler(...args) {
    // Optional confirmation
    if (confirmMessage && !confirm(confirmMessage)) {
      return { success: false, cancelled: true };
    }

    // Set loading state
    if (stateRefs.setLoading) {
      stateRefs.setLoading(true);
    }

    try {
      const result = await operation(...args);
      onSuccess?.(result);
      return { success: true, data: result };
    } catch (error) {
      const errorMessage = error.message || String(error);
      onError?.(errorMessage);
      return { success: false, error: errorMessage };
    } finally {
      if (stateRefs.setLoading) {
        stateRefs.setLoading(false);
      }
    }
  };
}
