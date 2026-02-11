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
};

// ===== API Endpoints =====
export const API = {
  health: '/api/health',
  connection: '/api/connection',
  people: '/api/people',
  progress: '/api/progress',
  start: '/api/start',
  cancel: '/api/cancel',
  output: '/api/output',
  config: '/api/config',
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

// ===== Default Configuration =====
// Must match backend defaults in config.rs
export const DEFAULT_CONFIG = {
  processing: {
    max_workers: 4,
    face_resolution: {
      enabled: true,
      min_size: 80,
    },
    blur: {
      enabled: false,
      min_sharpness: 15.0,
    },
    brightness: {
      enabled: false,
      min_brightness: 0.1,
      max_brightness: 0.95,
    },
    head_pose: {
      enabled: true,
      max_yaw: 35.0,
      max_pitch: 35.0,
      max_roll: 25.0,
    },
    eye_filter: {
      enabled: false,
      min_ear: 0.2,
    },
    output: {
      size: 512,
      keep_intermediates: false,
    },
    alignment: {
      eye_distance: 0.3,
    },
    timestamp: {
      enabled: false,
      position: 'bottom_left',
      year: true,
      month: false,
      day: false,
    },
    time_interval: {
      enabled: false,
      max_photos: 1,
      time_range: 'day',
    },
  },
  video: {
    enabled: true,
    framerate: 15,
    codec: 'libx264',
    crf: 23,
  },
};
