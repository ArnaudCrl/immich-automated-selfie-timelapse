/**
 * Application constants and magic values.
 */

// ===== Timing (milliseconds) =====
export const TIMING = {
  messageDisplayDuration: 2000,
};

// ===== Local Storage Keys =====
export const STORAGE_KEYS = {
  selectedPerson: 'immich-timelapse-selected-person',
  selectedAlbums: 'immich-timelapse-selected-albums',
};

// ===== API Endpoints =====
export const API = {
  health: '/api/health',
  connection: '/api/connection',
  people: '/api/people',
  albums: '/api/albums',
  progress: '/api/progress',
  start: '/api/start',
  cancel: '/api/cancel',
  output: '/api/output',
  config: '/api/config',
  configDefaults: '/api/config/defaults',
  ws: '/api/ws',
};

// ===== WebSocket =====
export const WS = {
  reconnectDelay: 1000,
  getUrl: () => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${API.ws}`;
  },
};

// ===== Job Status =====
export const JOB_STATUS = {
  idle: 'idle',
  running: 'running',
  compiling: 'compiling_video',
  cancelling: 'cancelling',
  completed: 'completed',
  cancelled: 'cancelled',
  error: 'error',
};

