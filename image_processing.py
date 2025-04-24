# image_processing.py
import os
import io
import concurrent.futures
from datetime import datetime
from dataclasses import dataclass
from PIL import Image, ImageOps
import numpy as np
import cv2
import dlib
from tqdm import tqdm
import logging
from typing import Tuple
from immich_api import get_assets_with_person, download_asset

SUPPORTED_IMAGE_MIME_TYPES = {
    "image/jpeg",
    "image/jpg",
    "image/png",
    "image/gif",
    "image/webp",
}

class TqdmLoggingHandler(logging.Handler):
    def __init__(self, level=logging.NOTSET):
        super().__init__(level)

    def emit(self, record):
        try:
            msg = self.format(record)
            tqdm.write(msg)
        except Exception:
            self.handleError(record)


logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)
tqdm_handler = TqdmLoggingHandler()
formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s', datefmt='%H:%M:%S')
tqdm_handler.setFormatter(formatter)
logger.addHandler(tqdm_handler)


@dataclass
class AppConfig:
    """Configuration for the application."""
    api_key: str
    base_url: str
    person_id: str
    output_folder: str
    landmark_model: str
    resize_size: int
    face_resolution_threshold: int
    pose_threshold: float
    left_eye_pos: Tuple[float, float]
    date_from: str
    date_to: str


def initialize_worker(landmark_model_path: str) -> None:
    """Initialize worker process with face predictor.
    
    Args:
        landmark_model_path: Path to the landmark model file
    """
    global face_predictor
    face_predictor = dlib.shape_predictor(landmark_model_path)


def detect_landmarks(image, face_data, face_resolution_threshold):
    """
    Detects facial landmarks in the image, resizing if necessary for better detection.

    Args:
        image (PIL.Image): The input image.
        face_data (dict): Face metadata containing bounding box information.
        max_landmark_size (int): Maximum size for landmark detection.

    Returns:
        dict or None: Dictionary containing facial landmarks in numpy arrays if successful,
                     None if face resolution is too low.
    """
    # Get face rectangle from metadata with proper scaling
    face_img_width = face_data.get("imageWidth")
    face_img_height = face_data.get("imageHeight")
    img_width, img_height = image.size
    scale_x = img_width / face_img_width
    scale_y = img_height / face_img_height
    x1 = int(face_data.get("boundingBoxX1", 0) * scale_x)
    x2 = int(face_data.get("boundingBoxX2", 0) * scale_x)
    y1 = int(face_data.get("boundingBoxY1", 0) * scale_y)
    y2 = int(face_data.get("boundingBoxY2", 0) * scale_y)
    w = x2 - x1
    h = y2 - y1
    
    if w < face_resolution_threshold or h < face_resolution_threshold:
        return None

    # Crop the face region from the image
    face_crop = image.crop((x1, y1, x2, y2))
    
    # If the cropped face is too large, resize it for better landmark detection
    optimal_size = 256  #  optimal size for landmark detection
    scale_factor = 1.0
    if w > optimal_size or h > optimal_size:
        scale_factor = optimal_size / max(w, h)
        new_w = int(w * scale_factor)
        new_h = int(h * scale_factor)
        face_crop = face_crop.resize((new_w, new_h), Image.Resampling.LANCZOS)
        w, h = new_w, new_h

    # Convert PIL Image to numpy array for OpenCV processing
    img_np = np.array(face_crop)
    img_np = cv2.cvtColor(img_np, cv2.COLOR_RGB2BGR)

    # Create dlib rectangle spanning the whole cropped image
    face_rect = dlib.rectangle(0, 0, w, h)

    # Detect facial landmarks
    shape = face_predictor(img_np, face_rect)
    if not shape:
        return None

    # Scale landmarks back to original image coordinates and adjust for face rectangle position
    landmarks = {}
    for i in range(68):
        x = shape.part(i).x
        y = shape.part(i).y
        if scale_factor != 1.0:
            x = int(x / scale_factor)
            y = int(y / scale_factor)
        # Add the face rectangle's top-left coordinates to position landmarks correctly
        x += x1
        y += y1
        landmarks[i] = (x, y)

    # Convert to numpy arrays for specific facial features
    left_eye = np.array([landmarks[i] for i in range(36, 42)])
    right_eye = np.array([landmarks[i] for i in range(42, 48)])
    nose_tip = np.array(landmarks[30])
    chin = np.array(landmarks[8])
    left_mouth = np.array(landmarks[48])
    right_mouth = np.array(landmarks[54])

    return {
        'left_eye': left_eye,
        'right_eye': right_eye,
        'nose_tip': nose_tip,
        'chin': chin,
        'left_mouth': left_mouth,
        'right_mouth': right_mouth,
        'all_landmarks': landmarks
    }


def check_eye_visibility(left_eye, right_eye, ear_threshold=0.2) -> bool:
    """
    Checks if both eyes are visible by calculating the Eye Aspect Ratio (EAR).

    Args:
        left_eye (numpy.ndarray): Array of left eye landmarks.
        right_eye (numpy.ndarray): Array of right eye landmarks.
        ear_threshold (float): Threshold for eye visibility.

    Returns:
        bool: True if both eyes are open enough, False otherwise.
    """
    def calculate_ear(eye):
        v1 = np.linalg.norm(eye[1] - eye[5])
        v2 = np.linalg.norm(eye[2] - eye[4])
        h = np.linalg.norm(eye[0] - eye[3])
        ear = (v1 + v2) / (2.0 * h)
        return ear

    left_ear = calculate_ear(left_eye)
    right_ear = calculate_ear(right_eye)

    if left_ear < ear_threshold or right_ear < ear_threshold:
        return False

    return True


def get_head_pose(landmarks, image):
    """
    Estimates the head pose (pitch, yaw, roll) using facial landmarks.
    Based on https://learnopencv.com/head-pose-estimation-using-opencv-and-dlib/

    Args:
        landmarks (dict): Dictionary containing facial landmarks in numpy arrays.
        image (PIL.Image): The input image.

    Returns:
        tuple or None: (pitch, yaw, roll) in degrees if successful; otherwise None.
    """
    # Get image size
    img_width, img_height = image.size

    image_points = np.array([
        landmarks['nose_tip'],
        landmarks['chin'],
        landmarks['left_eye'][0],
        landmarks['right_eye'][3],
        landmarks['left_mouth'],
        landmarks['right_mouth']
    ], dtype="double")

    model_points = np.array([
        (0.0, 0.0, 0.0),  # Nose tip
        (0.0, -330.0, -65.0),  # Chin
        (-225.0, 170.0, -135.0),  # Left eye left corner
        (225.0, 170.0, -135.0),  # Right eye right corner
        (-150.0, -150.0, -125.0),  # Left Mouth corner
        (150.0, -150.0, -125.0)  # Right mouth corner
    ])

    focal_length = img_width
    center = (img_width / 2, img_height / 2)
    camera_matrix = np.array(
        [[focal_length, 0, center[0]],
         [0, focal_length, center[1]],
         [0, 0, 1]], dtype="double"
    )
    dist_coeffs = np.zeros((4, 1))
    success, rotation_vector, translation_vector = cv2.solvePnP(
        model_points, image_points, camera_matrix, dist_coeffs, flags=cv2.SOLVEPNP_ITERATIVE
    )
    if not success:
        logger.info("Head pose estimation failed in solvePnP.")
        return None
    rotation_matrix, _ = cv2.Rodrigues(rotation_vector)
    proj_matrix = np.hstack((rotation_matrix, translation_vector))
    _, _, _, _, _, _, euler_angles = cv2.decomposeProjectionMatrix(proj_matrix)
    pitch, yaw, roll = [float(angle) for angle in euler_angles]

    # Normalize angles to be between -180 and 180 degrees
    pitch = (pitch + 180) % 360 - 180
    yaw = (yaw + 180) % 360 - 180
    roll = (roll + 180) % 360 - 180

    # Adjust pitch to be between -90 and 90 degrees
    if pitch > 90:
        pitch = 180 - pitch
    elif pitch < -90:
        pitch = -180 - pitch

    return pitch, yaw, roll


def calculate_eye_alignment_transform(
    left_eye_center: np.ndarray,
    right_eye_center: np.ndarray,
    output_size: int,
    desired_left_eye_pos: Tuple[float, float]
) -> np.ndarray:
    """Calculate the transformation matrix to align eyes at desired positions.
    
    Args:
        left_eye_center: Center coordinates of the left eye
        right_eye_center: Center coordinates of the right eye
        output_size: Size of the output image (width and height)
        desired_left_eye_pos: Desired position of left eye as percentages (x, y)
        
    Returns:
        np.ndarray: 2x3 transformation matrix for cv2.warpAffine
    """
    # Calculate the desired eye positions in the output image
    left_eye_target = np.array([
        output_size * desired_left_eye_pos[0],
        output_size * desired_left_eye_pos[1]
    ])
    right_eye_target = np.array([
        output_size * (1.0 - desired_left_eye_pos[0]),
        output_size * desired_left_eye_pos[1]
    ])

    # Calculate the angle between the current eye line and the target eye line
    current_angle = np.degrees(np.arctan2(
        right_eye_center[1] - left_eye_center[1],
        right_eye_center[0] - left_eye_center[0]
    ))
    target_angle = np.degrees(np.arctan2(
        right_eye_target[1] - left_eye_target[1],
        right_eye_target[0] - left_eye_target[0]
    ))
    rotation_angle = target_angle - current_angle

    # Calculate the scale factor to match the desired eye distance
    current_eye_distance = np.linalg.norm(right_eye_center - left_eye_center)
    target_eye_distance = np.linalg.norm(right_eye_target - left_eye_target)
    scale = target_eye_distance / current_eye_distance

    # Create the transformation matrix
    # First, translate to origin (center of eyes)
    center = np.array([
        (left_eye_center[0] + right_eye_center[0]) / 2,
        (left_eye_center[1] + right_eye_center[1]) / 2
    ])
    M1 = np.array([
        [1, 0, -center[0]],
        [0, 1, -center[1]],
        [0, 0, 1]
    ])

    # Then rotate
    angle_rad = np.radians(rotation_angle)
    M2 = np.array([
        [np.cos(angle_rad), -np.sin(angle_rad), 0],
        [np.sin(angle_rad), np.cos(angle_rad), 0],
        [0, 0, 1]
    ])

    # Then scale
    M3 = np.array([
        [scale, 0, 0],
        [0, scale, 0],
        [0, 0, 1]
    ])

    # Finally, translate to target position
    target_center = np.array([
        (left_eye_target[0] + right_eye_target[0]) / 2,
        (left_eye_target[1] + right_eye_target[1]) / 2
    ])
    M4 = np.array([
        [1, 0, target_center[0]],
        [0, 1, target_center[1]],
        [0, 0, 1]
    ])

    # Combine all transformations
    M = M4 @ M3 @ M2 @ M1

    # Convert to 2x3 matrix for OpenCV
    return M[:2, :]

def crop_and_align_face(image, face_data, resize_size, face_resolution_threshold, pose_threshold, left_eye_pos):
    """
    Aligns a face in an image by positioning the eyes at specified locations.

    Args:
        image (PIL.Image): The input image.
        face_data (dict): Face metadata containing bounding box information.
        resize_size (int): Size to resize the output image to.
        face_resolution_threshold (int): Minimum face resolution threshold.
        pose_threshold (float): Maximum allowed head pose deviation.
        left_eye_pos (tuple): Desired position of the left eye in the output as percentages (x, y).

    Returns:
        PIL.Image or None: The aligned face image if successful, None otherwise.
    """
    try:
        # Convert image to numpy array for OpenCV processing
        img_np = np.array(image)
        img_np = cv2.cvtColor(img_np, cv2.COLOR_RGB2BGR)

        # Detect landmarks in the face region
        landmarks = detect_landmarks(image, face_data, face_resolution_threshold)
        if not landmarks:
            logger.info("Face resolution is too low")
            return None

        # Check if both eyes are visible
        if not check_eye_visibility(landmarks['left_eye'], landmarks['right_eye']):
            logger.info("Eyes are too closed")
            return None

        # Get head pose
        pose = get_head_pose(landmarks, image)
        if not pose:
            logger.info("Could not estimate head pose")
            return None

        # Check if head pose is within acceptable range
        pitch, yaw, roll = pose
        if  abs(yaw) > pose_threshold:
            logger.info(f"Head pose exceeds threshold: pitch={pitch:.1f}°, yaw={yaw:.1f}°, roll={roll:.1f}°")
            return None

        # Get eye positions
        left_eye_center = np.mean(landmarks['left_eye'], axis=0)
        right_eye_center = np.mean(landmarks['right_eye'], axis=0)

        # Calculate transformation matrix
        rotation_matrix = calculate_eye_alignment_transform(
            left_eye_center,
            right_eye_center,
            resize_size,
            left_eye_pos
        )

        # Apply transformation
        aligned_face = cv2.warpAffine(
            img_np,
            rotation_matrix,
            (resize_size, resize_size),
            flags=cv2.INTER_LINEAR,
            borderMode=cv2.BORDER_REPLICATE
        )

        # Convert back to PIL Image
        aligned_face = cv2.cvtColor(aligned_face, cv2.COLOR_BGR2RGB)
        aligned_face = Image.fromarray(aligned_face)

        return aligned_face

    except Exception as e:
        logger.error(f"Error in crop_and_align_face: {str(e)}")
        return None


def process_asset_worker(asset, config: AppConfig):
    """
    Worker function to process a single asset.

    This function downloads the asset, crops the face based on metadata,
    verifies resolution, aligns the face, and then saves the aligned face.

    Args:
        asset (dict): The asset metadata.
        config (AppConfig): Configuration parameters.

    Returns:
        str or None: The file path of the saved image if processing is successful; otherwise None.
    """
    try:
        asset_id = asset['id']
        use_original = asset['originalMimeType'] in SUPPORTED_IMAGE_MIME_TYPES
        image_bytes = download_asset(config.api_key, config.base_url, asset_id, use_original)
        image = Image.open(io.BytesIO(image_bytes))
        image = ImageOps.exif_transpose(image)
        image = image.convert("RGB")
    except Exception as e:
        logger.info(f"Error processing asset {asset.get('id')}: {e}")
        return None

    matching_person = next((p for p in asset.get('people', []) if p.get('id') == config.person_id), None)
    face_data = matching_person.get('faces', [])[0]
    
    aligned_face = crop_and_align_face(
        image,
        face_data,
        resize_size=config.resize_size,
        face_resolution_threshold=config.face_resolution_threshold,
        pose_threshold=config.pose_threshold,
        left_eye_pos=config.left_eye_pos
    )

    if aligned_face is None:
        return None

    dt = datetime.fromisoformat(asset['fileCreatedAt'].replace("Z", "+00:00"))
    timestamp = dt.strftime("%Y%m%d_%H%M%S")
    filename = os.path.join(config.output_folder, f"{timestamp}.jpg")
    aligned_face.save(filename)
    return filename

def process_faces(config: AppConfig, max_workers=1, progress_callback=None, cancel_flag=None):
    """
    Processes assets containing the person and saves aligned face images.

    This function retrieves assets from the API, then uses a process pool to
    concurrently download, crop, and align faces.

    Args:
        config (AppConfig): Configuration parameters.
        max_workers (int): Number of worker processes.
        progress_callback (callable, optional): A callback function for progress updates.
        cancel_flag (callable, optional): A function that returns True if processing should be cancelled.

    Returns:
        list: A list of file paths of the saved images.
    """
    if cancel_flag and cancel_flag():
        logger.info("Processing was cancelled.")
        return []

    assets = get_assets_with_person(config.api_key, config.base_url, config.person_id, config.date_from, config.date_to)
    logger.info(f"Found {len(assets)} assets containing the person.")

    total_assets = len(assets)
    if progress_callback:
        progress_callback(0, total_assets)
    processed_files = []
    completed_count = 0

    initializer_args = [config.landmark_model]
    with concurrent.futures.ProcessPoolExecutor(
            max_workers=max_workers,
            initializer=initialize_worker,
            initargs=initializer_args) as executor:
        future_to_asset = {executor.submit(process_asset_worker, asset, config): asset
                           for asset in assets}
        for future in tqdm(concurrent.futures.as_completed(future_to_asset), total=total_assets):

            if cancel_flag and cancel_flag():
                logger.info("Processing was cancelled.")
                for f in future_to_asset:
                    f.cancel()
                executor.shutdown(wait=False)
                return processed_files

            try:
                result = future.result()
                if result is not None:
                    processed_files.append(result)
            except Exception as e:
                logger.info(f"Asset processing failed: {e}")

            completed_count += 1
            if progress_callback:
                progress_callback(completed_count, total_assets)

    logger.info(f"Finished processing. {len(processed_files)} images saved out of {total_assets} assets.")
    return processed_files