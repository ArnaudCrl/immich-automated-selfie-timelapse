/**
 * Standardized error handling utilities.
 * Provides consistent error display and logging across the application.
 */

/**
 * Parse error from fetch response or exception
 * @param {Error|Response} error - Error object or fetch response
 * @returns {Promise<string>} Error message
 */
export async function parseError(error) {
  // If it's a Response object, try to extract JSON error message
  if (error instanceof Response) {
    try {
      const data = await error.json();
      return data.message || `Request failed with status ${error.status}`;
    } catch {
      return `Request failed with status ${error.status}`;
    }
  }

  // If it's an Error object, use its message
  if (error instanceof Error) {
    return error.message;
  }

  // Fallback for unknown error types
  return String(error);
}

/**
 * Handle error with console logging
 * @param {string} context - Context where error occurred (e.g., "Failed to load images")
 * @param {Error|Response} error - Error object or response
 * @returns {Promise<string>} Formatted error message
 */
export async function handleError(context, error) {
  const message = await parseError(error);
  const fullMessage = `${context}: ${message}`;
  console.error(fullMessage, error);
  return fullMessage;
}

/**
 * Show error in alert dialog
 * @param {string} context - Context where error occurred
 * @param {Error|Response} error - Error object or response
 */
export async function showErrorAlert(context, error) {
  const message = await handleError(context, error);
  alert(message);
}

/**
 * Create a fetch wrapper with standardized error handling
 * @param {string} url - Fetch URL
 * @param {RequestInit} options - Fetch options
 * @returns {Promise<Response>} Response object if ok, throws on error
 */
export async function fetchWithErrorHandling(url, options = {}) {
  try {
    const response = await fetch(url, options);
    if (!response.ok) {
      throw response;
    }
    return response;
  } catch (error) {
    // Re-throw for caller to handle
    throw error;
  }
}

/**
 * Safely parse JSON from response, with fallback
 * @param {Response} response - Fetch response
 * @param {any} fallback - Fallback value if parsing fails
 * @returns {Promise<any>} Parsed JSON or fallback
 */
export async function safeJsonParse(response, fallback = {}) {
  try {
    return await response.json();
  } catch {
    return fallback;
  }
}

/**
 * Error display modes
 */
export const ErrorMode = {
  ALERT: 'alert',
  CONSOLE: 'console',
  INLINE: 'inline',
};

/**
 * Generic async operation wrapper with error handling
 * @param {Function} operation - Async operation to execute
 * @param {Object} options - Options
 * @param {string} options.context - Error context message
 * @param {string} options.mode - Error display mode (alert, console, inline)
 * @param {Function} options.onError - Custom error handler
 * @param {Function} options.onSuccess - Success callback
 * @returns {Promise<{success: boolean, data?: any, error?: string}>}
 */
export async function executeAsync(operation, options = {}) {
  const {
    context = 'Operation failed',
    mode = ErrorMode.CONSOLE,
    onError,
    onSuccess,
  } = options;

  try {
    const result = await operation();
    onSuccess?.();
    return { success: true, data: result };
  } catch (error) {
    const errorMessage = await handleError(context, error);

    // Display error based on mode
    if (mode === ErrorMode.ALERT) {
      alert(errorMessage);
    }

    // Call custom error handler if provided
    onError?.(errorMessage);

    return { success: false, error: errorMessage };
  }
}
