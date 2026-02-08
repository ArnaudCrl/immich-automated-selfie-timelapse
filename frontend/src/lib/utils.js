/**
 * Shared utility functions for the frontend.
 * These must match backend logic where noted.
 */

/**
 * Sanitize a folder name to match backend's sanitize_folder_name logic.
 * IMPORTANT: Must exactly replicate src/utils.rs sanitize_folder_name
 *
 * @param {string} name - Person name
 * @param {string} id - Person ID (fallback)
 * @returns {string} Sanitized folder name
 */
export function sanitizeFolderName(name, id) {
  const base = name && name.trim() ? name : id;

  // Remove accents by decomposing to NFD and filtering combining marks
  const withoutAccents = base.normalize('NFD').replace(/[\u0300-\u036f]/g, '');

  // Convert to lowercase and replace unsafe characters with underscores
  let sanitized = '';
  for (const c of withoutAccents.toLowerCase()) {
    if (/^[a-z0-9\-_]$/.test(c)) {
      sanitized += c;
    } else {
      sanitized += '_';
    }
  }

  // Trim whitespace and underscores, then limit length
  sanitized = sanitized.replace(/^[\s_]+|[\s_]+$/g, '');
  return sanitized.length > 50 ? sanitized.slice(0, 50) : sanitized;
}

/**
 * Format bytes to human-readable size string.
 *
 * @param {number} bytes - Size in bytes
 * @returns {string} Formatted size (e.g., "1.2 MB")
 */
export function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
