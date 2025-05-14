import os
from dataclasses import dataclass
from typing import Tuple

@dataclass
class AppConfig:
    """Configuration for the application."""
    # This
    api_key: str = os.environ.get("IMMICH_API_KEY", "")
    base_url: str = os.environ.get("IMMICH_BASE_URL", "")
    output_folder: str = os.environ.get("OUTPUT_FOLDER", "output")
    landmark_model: str = os.environ.get("LANDMARK_MODEL", "shape_predictor_68_face_landmarks.dat")
    # That
    person_id: str = os.environ.get("DEFAULT_PERSON_ID", "")
    resize_size: int = int(os.environ.get("DEFAULT_RESIZE_SIZE", 512))
    face_resolution_threshold: int = int(os.environ.get("DEFAULT_FACE_RESOLUTION_THRESHOLD", 80))
    pose_threshold: float = float(os.environ.get("DEFAULT_POSE_THRESHOLD", 25.0))
    date_from: str = os.environ.get("DEFAULT_DATE_FROM", None)
    date_to: str = os.environ.get("DEFAULT_DATE_TO", None)
    ear_threshold: float = float(os.environ.get("DEFAULT_EAR_THRESHOLD", 0.2))
    max_workers: int = int(os.environ.get("DEFAULT_MAX_WORKERS", 1))
    do_not_compile_video: bool = os.environ.get("DEFAULT_DO_NOT_COMPILE_VIDEO", False)
    framerate: int = int(os.environ.get("DEFAULT_FRAMERATE", 15))
    # Hmm
    left_eye_pos: Tuple[float, float] = (0.35, 0.4)