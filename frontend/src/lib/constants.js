/**
 * Application constants and magic values.
 * Single source of truth for colors, spacing, timing, etc.
 */

// ===== Colors =====
export const COLORS = {
  // Backgrounds
  bgPrimary: '#0f0f0f',
  bgSecondary: '#1a1a1a',
  bgTertiary: '#252525',
  bgHover: '#333',
  bgHoverLight: '#444',

  // Text
  textPrimary: '#fff',
  textSecondary: '#e0e0e0',
  textTertiary: '#888',
  textQuaternary: '#666',

  // Borders
  border: '#333',
  borderLight: '#252525',

  // Brand/Accent
  primary: '#4f46e5',
  primaryHover: '#4338ca',

  // Status colors
  success: '#22c55e',
  error: '#dc2626',
  warning: '#f59e0b',
  info: '#3b82f6',
};

// ===== Spacing =====
export const SPACING = {
  xs: '0.25rem',
  sm: '0.5rem',
  md: '0.75rem',
  lg: '1rem',
  xl: '1.5rem',
  xxl: '2rem',
  xxxl: '3rem',
};

// ===== Border Radius =====
export const RADIUS = {
  sm: '4px',
  md: '6px',
  lg: '8px',
};

// ===== Transitions =====
export const TRANSITIONS = {
  default: 'all 0.15s ease',
  fast: 'all 0.1s ease',
  slow: 'all 0.3s ease',
};

// ===== Font Sizes =====
export const FONT_SIZES = {
  xs: '0.625rem',
  sm: '0.75rem',
  md: '0.875rem',
  base: '1rem',
  lg: '1.125rem',
  xl: '1.5rem',
};

// ===== Z-Index Layers =====
export const Z_INDEX = {
  base: 0,
  dropdown: 10,
  overlay: 100,
  modal: 1000,
  toast: 10000,
};

// ===== Timing (milliseconds) =====
export const TIMING = {
  messageDisplayDuration: 2000,
  wsReconnectDelay: 1000,
  debounceDefault: 300,
  animationFast: 150,
  animationDefault: 300,
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
    photo_limit: {
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
