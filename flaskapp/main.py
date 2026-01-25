import logging
import os
from flask import Flask, session, jsonify, redirect, url_for, send_from_directory, request, g
from flask_cors import CORS
from functools import wraps
from datetime import datetime

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

app = Flask(__name__)
app.secret_key = 'your-secret-key-here'  # TODO: replace with a real secret key

CORS(app, origins=["http://localhost:5173"], supports_credentials=True)

def is_logged_in():
    print("session dict:", dict(session))
    print("Session IMMICH_BASE_URL:", session.get("IMMICH_BASE_URL"))
    print("Session IMMICH_API_KEY:", session.get("IMMICH_API_KEY"))
    print("Env IMMICH_BASE_URL:", os.getenv("IMMICH_BASE_URL"))
    print("Env IMMICH_API_KEY:", os.getenv("IMMICH_API_KEY"))
    if session.get("IMMICH_BASE_URL") and session.get("IMMICH_API_KEY"):
        return validate_api_credentials(
            base_url=session.get("IMMICH_BASE_URL"),
            api_key=session.get("IMMICH_API_KEY")
        )
    else:
        return validate_api_credentials(
            base_url=os.getenv("IMMICH_BASE_URL"),
            api_key=os.getenv("IMMICH_API_KEY")
        )

def login_required(f):
    @wraps(f)
    def decorated_function(*args, **kwargs):
        if is_logged_in():
            return f(*args, **kwargs)
        else:
            return "Unauthorized", 401
    return decorated_function

@app.route("/login", methods=["GET", "POST"])
def login():
    if request.method == "GET":
        if session.get("IMMICH_BASE_URL") and session.get("IMMICH_API_KEY"):
            return jsonify({
                "IMMICH_BASE_URL": session.get("IMMICH_BASE_URL"),
                "IMMICH_API_KEY": session.get("IMMICH_API_KEY"),
                "isLoggedIn": is_logged_in()
            })
        else:
            return jsonify({
                "IMMICH_BASE_URL": os.getenv("IMMICH_BASE_URL"),
                "IMMICH_API_KEY": os.getenv("IMMICH_API_KEY"),
                "isLoggedIn": is_logged_in()
            })
    elif request.method == "POST":
        base_url = request.form.get("baseUrl", "").strip()
        api_key = request.form.get("apiKey", "").strip()
        if validate_base_url(base_url):
            base_url = validate_base_url(base_url)
            if validate_api_credentials(base_url, api_key):
                session['IMMICH_BASE_URL'] = base_url
                session['IMMICH_API_KEY'] = api_key
                return jsonify({"error": False}), 200
            return jsonify({"error": True, "message": "Server reachable but API key denied", "field": "apiKey"}), 401
        return jsonify({"error": True, "message": "Server unreachable", "field": "baseUrl"}), 401


def get_v(key: str, default=None):
    return session.get(key, os.getenv(key, default))

@app.route("/settings", methods=["GET", "POST"])
@login_required
def settings():
    if request.method == "GET":
        return jsonify({
            "searchTakenAfter": get_v("SEARCH_TAKEN_AFTER", "1970-01-01"),
            "searchTakenBefore": get_v("SEARCH_TAKEN_BEFORE", datetime.today().strftime("%Y-%m-%d")),
            "searchAlbumId": get_v("SEARCH_ALBUM_ID", ""),
            "searchIsFavorite": get_v("SEARCH_IS_FAVORITE", False),

            "keep": get_v("KEEP", True),
            "keepHourly": int(get_v("KEEP_HOURLY", 1)),
            "keepDaily": int(get_v("KEEP_DAILY", 3)),
            "keepWeekly": int(get_v("KEEP_WEEKLY", 10)),
            "keepMonthly": int(get_v("KEEP_MONTHLY", 20)),
            "keepYearly": int(get_v("KEEP_YEARLY", 50)),

            "faceResizeSize": int(get_v("FACE_RESIZE_SIZE", 520)),
            "faceResolutionThreshold": int(get_v("FACE_RESOLUTION_THRESHOLD", 80)),
            "facePaddingRatio": float(get_v("FACE_PADDING_RATIO", 1.2)),

            "label": get_v("LABEL", True),
            "labelSize": int(get_v("LABEL_SIZE", 12)),
            "labelPos": get_v("LABEL_POS", "bottom_left"),
            "labelFormat": get_v("LABEL_FORMAT", "%Y-%m-%d"),
            "labelPadding": int(get_v("LABEL_PADDING", 10)),
            "labelFill": get_v("LABEL_FILL", "#FFFFFF"),

            "videoCompile": get_v("VIDEO_COMPILE", True),
            "videoFramerate": int(get_v("VIDEO_FRAMERATE", 15)),
            "videoWorkers": int(get_v("VIDEO_WORKERS", 4)),
        })
    elif request.method == "POST":
        session['SEARCH_TAKEN_AFTER'] = request.form.get("searchTakenAfter")
        session['SEARCH_TAKEN_BEFORE'] = request.form.get("searchTakenBefore")
        session['SEARCH_ALBUM_ID'] = request.form.get("searchAlbumId", "")
        session['SEARCH_IS_FAVORITE'] = request.form.get("searchIsFavorite") == "true"
        session['KEEP'] = request.form.get("keep") == "true"
        session['KEEP_HOURLY'] = int(request.form.get("keepHourly"))
        session['KEEP_DAILY'] = int(request.form.get("keepDaily"))
        session['KEEP_WEEKLY'] = int(request.form.get("keepWeekly"))
        session['KEEP_MONTHLY'] = int(request.form.get("keepMonthly"))
        session['KEEP_YEARLY'] = int(request.form.get("keepYearly"))
        session['FACE_RESIZE_SIZE'] = int(request.form.get("faceResizeSize"))
        session['FACE_RESOLUTION_THRESHOLD'] = int(request.form.get("faceResolutionThreshold"))
        session['FACE_PADDING_RATIO'] = float(request.form.get("facePaddingRatio"))
        session['LABEL'] = request.form.get("label") == "true"
        session['LABEL_SIZE'] = int(request.form.get("labelSize"))
        session['LABEL_POS'] = request.form.get("labelPos")
        session['LABEL_FORMAT'] = request.form.get("labelFormat")
        session['LABEL_PADDING'] = int(request.form.get("labelPadding"))
        session['LABEL_FILL'] = request.form.get("labelFill")
        session['VIDEO_COMPILE'] = request.form.get("videoCompile") == "true"
        session['VIDEO_FRAMERATE'] = int(request.form.get("videoFramerate"))
        session['VIDEO_WORKERS'] = int(request.form.get("videoWorkers"))
        return jsonify({"success": True}), 200


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
