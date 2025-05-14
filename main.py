import logging
import os
import threading
from typing import Callable, Dict, List, Tuple
from flask import Flask, jsonify, render_template

from image_processing import process_faces
from compile_timelapse import compile_timelapse
from forms import TimelapseForm, OUTPUT_FOLDER


# Filter out progress route logs
class ProgressRouteFilter(logging.Filter):
    def filter(self, record: logging.LogRecord) -> bool:
        return "/progress" not in record.getMessage()

log = logging.getLogger('werkzeug')
log.addFilter(ProgressRouteFilter())

# Initialize Flask app
app = Flask(__name__)
app.config['SECRET_KEY'] = "dev"

# Global state
PROGRESS_INFO: Dict[str, any] = {"completed": 0, "total": 0, "status": "idle"}
PROCESSING_THREAD: threading.Thread = None
CANCEL_REQUESTED: bool = False

def update_progress(current: int, total: int) -> None:
    """Update the global progress information.
    
    Args:
        current: Number of completed tasks
        total: Total number of tasks
    """
    PROGRESS_INFO["completed"] = current
    PROGRESS_INFO["total"] = total
    PROGRESS_INFO["status"] = "running" if current < total else "done"

def check_output_folder(output_folder) -> Tuple[bool, int]:
    """Check if the output folder is empty.
    
    Returns:
        Tuple containing (is_empty, file_count)
    """
    if not os.path.exists(output_folder):
        os.makedirs(output_folder, exist_ok=True)
        return True, 0

    files = [f for f in os.listdir(output_folder) 
             if os.path.isfile(os.path.join(output_folder, f))]
    return len(files) == 0, len(files)

def background_process(
    config: dict,
    progress_callback: Callable = None,
    cancel_flag: Callable = None,
) -> List[str]:
    """Process faces in the background and optionally compile a timelapse video.
    
    Args:
        progress_callback: Optional callback for progress updates
        cancel_flag: Optional function to check for cancellation
    """
    try:
        # Process faces
        process_faces(
            config=config,
            progress_callback=progress_callback,
            cancel_flag=cancel_flag
        )

        if progress_callback:
                progress_callback(1, 1)
        
        if not config["do_not_compile_video"] and not cancel_flag():
            PROGRESS_INFO["status"] = "compiling_video"
            video_output_path = os.path.join(OUTPUT_FOLDER, "timelapse.mp4")
            success = compile_timelapse(
                image_folder=OUTPUT_FOLDER,
                output_path=video_output_path,
                framerate=config["framerate"],
                update_progress=progress_callback
            )
            PROGRESS_INFO["status"] = "video_done" if success else "error:Video compilation failed"

    except Exception as e:
        raise

@app.route("/progress")
def progress() -> Dict[str, any]:
    """Get current progress information."""
    return jsonify(PROGRESS_INFO)

@app.route("/cancel", methods=["POST"])
def cancel() -> Dict[str, any]:
    """Cancel the current processing job."""
    global PROCESSING_THREAD, CANCEL_REQUESTED
    CANCEL_REQUESTED = True
    if PROCESSING_THREAD and PROCESSING_THREAD.is_alive():
        PROGRESS_INFO["status"] = "cancelled"
        return jsonify({"success": True, "message": "Processing cancelled."})
    CANCEL_REQUESTED = False
    return jsonify({"success": False, "message": "No active processing to cancel."})

@app.route("/", methods=["GET", "POST"])
def index() -> str:
    """Handle the main page and processing requests."""
    global PROCESSING_THREAD, CANCEL_REQUESTED

    form = TimelapseForm()

    message = None
    error = None
    warning = None

    # Check output folder status
    is_empty, file_count = check_output_folder(OUTPUT_FOLDER)
    if not is_empty:
        warning = f"Output folder is not empty. Contains {file_count} files. New images will be added to this folder."

    if form.validate_on_submit():
        try:
            CANCEL_REQUESTED = False

            # Reset progress info
            PROGRESS_INFO.update({
                "completed": 0,
                "total": 0,
                "status": "idle"
            })

            # Start processing
            PROCESSING_THREAD = threading.Thread(
                target=background_process,
                kwargs={
                    "progress_callback": update_progress,
                    "cancel_flag": lambda: CANCEL_REQUESTED,
                    "config": form.data,
                }
            )
            PROCESSING_THREAD.start()

            message = "Processing started. Please wait and watch the progress bar below."

        except Exception as e:
            error = f"Error processing request: {e}"

    return render_template("index.html",
                           form=form,
                           message=message,
                           error=error,
                           warning=warning)

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=5000, debug=False)
