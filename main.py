import logging
import multiprocessing
import os
import threading
from dataclasses import dataclass
from typing import Callable, Dict, List, Tuple

from flask import Flask, jsonify, render_template, request
from image_processing import process_faces
from immich_api import validate_immich_connection
from compile_timelapse import compile_timelapse


# Filter out progress route logs
class ProgressRouteFilter(logging.Filter):
    def filter(self, record: logging.LogRecord) -> bool:
        return "/progress" not in record.getMessage()

log = logging.getLogger('werkzeug')
log.addFilter(ProgressRouteFilter())

@dataclass
class AppConfig:
    """Configuration for the application."""
    api_key: str = os.environ.get("IMMICH_API_KEY", "")
    base_url: str = os.environ.get("IMMICH_BASE_URL", "")
    person_id: str = None
    output_folder: str = "output"
    landmark_model: str = "shape_predictor_68_face_landmarks.dat"
    resize_size: int = 512
    face_resolution_threshold: int = 80
    pose_threshold: float = 25.0
    left_eye_pos: Tuple[float, float] = (0.35, 0.4)
    date_from: str = None
    date_to: str = None
    ear_threshold: float = 0.2

# Initialize Flask app
app = Flask(__name__)

# Global state
AVAILABLE_CORES = multiprocessing.cpu_count()
progress_info: Dict[str, any] = {"completed": 0, "total": 0, "status": "idle"}
processing_thread: threading.Thread = None
cancel_requested: bool = False
config = AppConfig()

def update_progress(current: int, total: int) -> None:
    """Update the global progress information.
    
    Args:
        current: Number of completed tasks
        total: Total number of tasks
    """
    progress_info["completed"] = current
    progress_info["total"] = total
    progress_info["status"] = "running" if current < total else "done"

def check_output_folder() -> Tuple[bool, int]:
    """Check if the output folder is empty.
    
    Returns:
        Tuple containing (is_empty, file_count)
    """
    if not os.path.exists(config.output_folder):
        os.makedirs(config.output_folder, exist_ok=True)
        return True, 0

    files = [f for f in os.listdir(config.output_folder) 
             if os.path.isfile(os.path.join(config.output_folder, f))]
    return len(files) == 0, len(files)

def background_process(
    max_workers: int = 1,
    progress_callback: Callable = None,
    cancel_flag: Callable = None,
    do_not_compile_video: bool = False,
    framerate: int = 15
) -> List[str]:
    """Process faces in the background and optionally compile a timelapse video.
    
    Args:
        max_workers: Number of worker processes for face processing
        progress_callback: Optional callback for progress updates
        cancel_flag: Optional function to check for cancellation
        do_not_compile_video: Whether to not compile a timelapse video after processing
        framerate: Frames per second for the output video
    """
    try:
        # Process faces
        process_faces(
            config=config,
            max_workers=max_workers,
            progress_callback=progress_callback,
            cancel_flag=cancel_flag
        )

        if progress_callback:
                progress_callback(1, 1)
        
        if not do_not_compile_video and not cancel_flag():
            progress_info["status"] = "compiling_video"
            video_output_path = os.path.join(config.output_folder, "timelapse.mp4")
            success = compile_timelapse(
                image_folder=config.output_folder,
                output_path=video_output_path,
                framerate=framerate,
                update_progress=progress_callback
            )
            progress_info["status"] = "video_done" if success else "error:Video compilation failed"

    except Exception as e:
        raise

@app.route("/progress")
def progress() -> Dict[str, any]:
    """Get current progress information."""
    return jsonify(progress_info)

@app.route("/check-connection")
def check_connection() -> Dict[str, any]:
    """Check connection to Immich server."""
    is_valid, message = validate_immich_connection(config.api_key, config.base_url)
    return jsonify({"valid": is_valid, "message": message})

@app.route("/cancel", methods=["POST"])
def cancel() -> Dict[str, any]:
    """Cancel the current processing job."""
    global processing_thread, cancel_requested
    cancel_requested = True
    if processing_thread and processing_thread.is_alive():
        progress_info["status"] = "cancelled"
        return jsonify({"success": True, "message": "Processing cancelled."})
    cancel_requested = False
    return jsonify({"success": False, "message": "No active processing to cancel."})

@app.route("/", methods=["GET", "POST"])
def index() -> str:
    """Handle the main page and processing requests."""
    global processing_thread, cancel_requested

    message = None
    error = None
    warning = None

    # Check output folder status
    is_empty, file_count = check_output_folder()
    if not is_empty:
        warning = f"Output folder is not empty. Contains {file_count} files. New images will be added to this folder."

    # Validate connection on POST
    if request.method == "POST":
        is_valid, message = validate_immich_connection(config.api_key, config.base_url)
        if not is_valid:
            error = f"Immich server connection error: {message}"
            return render_template("index.html", error=error, warning=warning,
                                max_workers_options=list(range(1, AVAILABLE_CORES + 1)))

        try:
            cancel_requested = False

            # Get form data
            config.person_id = request.form["person_id"]
            config.resize_size = int(request.form.get("resize_size"))
            config.face_resolution_threshold = int(request.form.get("face_resolution_threshold"))
            config.pose_threshold = float(request.form.get("pose_threshold"))
            config.date_from = request.form.get("date_from")
            config.date_to = request.form.get("date_to")
            config.ear_threshold = float(request.form.get("ear_threshold", 0.2))
            max_workers = int(request.form.get("max_workers"))
            do_not_compile_video = request.form.get("do_not_compile_video") == "on"
            framerate = int(request.form.get("framerate", 15))

            # Reset progress info
            progress_info.update({
                "completed": 0,
                "total": 0,
                "status": "idle"
            })

            # Start processing
            processing_thread = threading.Thread(
                target=background_process,
                kwargs={
                    "max_workers": max_workers,
                    "progress_callback": update_progress,
                    "cancel_flag": lambda: cancel_requested,
                    "do_not_compile_video": do_not_compile_video,
                    "framerate": framerate
                }  
            )   
            processing_thread.start()
            
            message = "Processing started. Please wait and watch the progress bar below."

        except Exception as e:
            error = f"Error processing request: {e}"

    return render_template("index.html", 
                         message=message, 
                         error=error, 
                         warning=warning,
                         max_workers_options=list(range(1, AVAILABLE_CORES + 1)))

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=5000, debug=False)