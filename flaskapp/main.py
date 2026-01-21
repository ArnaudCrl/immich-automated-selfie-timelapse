import logging
import os
import threading
from typing import Callable, Dict, List, Tuple
from flask import Flask, jsonify, render_template, redirect, url_for, send_from_directory, request

from immich_api import validate_api_credentials, validate_base_url


# Filter out progress route logs
class ProgressRouteFilter(logging.Filter):
    def filter(self, record: logging.LogRecord) -> bool:
        return (
            "/progress" not in record.getMessage()
            and "/_app/" not in record.getMessage()
        )

log = logging.getLogger('werkzeug')
log.addFilter(ProgressRouteFilter())

IMMICH_BASE_URL = os.getenv("IMMICH_BASE_URL", "")
IMMICH_API_KEY = os.getenv("IMMICH_API_KEY", "") 

app = Flask(__name__)

@app.route("/", methods=["GET"])
def index():
    if validate_api_credentials(IMMICH_BASE_URL, IMMICH_API_KEY):
        return redirect(url_for("home"))
    else:
        return redirect(url_for("login"))

@app.route("/home", methods=["GET"])
def home():
    print(IMMICH_BASE_URL, IMMICH_API_KEY)
    if validate_api_credentials(IMMICH_BASE_URL, IMMICH_API_KEY):
        return send_from_directory("../svelte/build", "home.html")
    else:
        return redirect(url_for("login"))

@app.route("/login", methods=["GET", "POST"])
def login():
    global IMMICH_BASE_URL, IMMICH_API_KEY 
    if request.method == "POST":
        base_url = request.form.get("baseUrl", "").strip()
        api_key = request.form.get("apiKey", "").strip()
        print(f"Received login attempt with baseUrl: {base_url} and apiKey: {api_key}")
        if validate_api_credentials(base_url, api_key):
            IMMICH_BASE_URL = base_url
            IMMICH_API_KEY = api_key            
            return jsonify({"baseUrlIsValid": True, "apiKeyIsValid": True}), 200
        else:
            if validate_base_url(base_url):
                return jsonify({"baseUrlIsValid": True, "apiKeyIsValid": False}), 401
            return jsonify({"baseUrlIsValid": False, "apiKeyIsValid": False}), 401
    else:
        if validate_api_credentials(IMMICH_BASE_URL, IMMICH_API_KEY):
            return redirect(url_for("home"))
        else:
            return send_from_directory("../svelte/build", "login.html")

@app.route('/_app/<path:filename>')
def serve_app_files(filename):
    return send_from_directory('../svelte/build/_app', filename)


# def check_output_folder(output_folder) -> Tuple[bool, int]:
#     """Check if the output folder is empty.
    
#     Returns:
#         Tuple containing (is_empty, file_count)
#     """
#     if not os.path.exists(output_folder):
#         os.makedirs(output_folder, exist_ok=True)
#         return True, 0

#     files = [f for f in os.listdir(output_folder) 
#              if os.path.isfile(os.path.join(output_folder, f))]
#     return len(files) == 0, len(files)

# def background_process(
#     config: dict,
#     progress_callback: Callable = None,
#     cancel_flag: Callable = None,
# ) -> List[str]:
#     """Process faces in the background and optionally compile a timelapse video.
    
#     Args:
#         progress_callback: Optional callback for progress updates
#         cancel_flag: Optional function to check for cancellation
#     """
#     try:
#         # Process faces
#         process_faces(
#             config=config,
#             progress_callback=progress_callback,
#             cancel_flag=cancel_flag
#         )

#         if progress_callback:
#                 progress_callback(1, 1)
        
#         if not config["do_not_compile_video"] and not cancel_flag():
#             PROGRESS_INFO["status"] = "compiling_video"
#             video_output_path = os.path.join(OUTPUT_FOLDER, "timelapse.mp4")
#             success = compile_timelapse(
#                 image_folder=OUTPUT_FOLDER,
#                 output_path=video_output_path,
#                 framerate=config["framerate"],
#                 update_progress=progress_callback
#             )
#             PROGRESS_INFO["status"] = "video_done" if success else "error:Video compilation failed"

#     except Exception as e:
#         raise

# @app.route("/progress")
# def progress() -> Dict[str, any]:
#     """Get current progress information."""
#     return jsonify(PROGRESS_INFO)

# @app.route("/cancel", methods=["POST"])
# def cancel() -> Dict[str, any]:
#     """Cancel the current processing job."""
#     global PROCESSING_THREAD, CANCEL_REQUESTED
#     CANCEL_REQUESTED = True
#     if PROCESSING_THREAD and PROCESSING_THREAD.is_alive():
#         PROGRESS_INFO["status"] = "cancelled"
#         return jsonify({"success": True, "message": "Processing cancelled."})
#     CANCEL_REQUESTED = False
#     return jsonify({"success": False, "message": "No active processing to cancel."})

# @app.route("/", methods=["GET", "POST"])
# def index() -> str:
#     """Handle the main page and processing requests."""
#     global PROCESSING_THREAD, CANCEL_REQUESTED

#     form = TimelapseForm()

#     message = None
#     error = None
#     warning = None

#     # Check output folder status
#     is_empty, file_count = check_output_folder(OUTPUT_FOLDER)
#     if not is_empty:
#         warning = f"Output folder is not empty. Contains {file_count} files. New images will be added to this folder."

#     if form.validate_on_submit():
#         try:
#             CANCEL_REQUESTED = False

#             # Reset progress info
#             PROGRESS_INFO.update({
#                 "completed": 0,
#                 "total": 0,
#                 "status": "idle"
#             })

#             # Start processing
#             PROCESSING_THREAD = threading.Thread(
#                 target=background_process,
#                 kwargs={
#                     "progress_callback": update_progress,
#                     "cancel_flag": lambda: CANCEL_REQUESTED,
#                     "config": form.data,
#                 }
#             )
#             PROCESSING_THREAD.start()

#             message = "Processing started. Please wait and watch the progress bar below."

#         except Exception as e:
#             error = f"Error processing request: {e}"
#     else:
#         if form.errors:
#             error = "Form validation failed. Please check your input."

#     return render_template("page1.html",
#                            form=form,
#                            message=message,
#                            error=error,
#                            warning=warning)

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=5000, debug=False)
