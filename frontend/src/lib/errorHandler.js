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
  // If it's a Response object, try to extract the error message
  if (error instanceof Response) {
    try {
      const text = await error.text();
      // Try parsing as JSON first (for structured error responses)
      try {
        const data = JSON.parse(text);
        return data.message || text || `Request failed with status ${error.status}`;
      } catch {
        // Plain text response from the backend
        return text || `Request failed with status ${error.status}`;
      }
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

