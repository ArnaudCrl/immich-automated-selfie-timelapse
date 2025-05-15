import requests
from os import environ
from multiprocessing import cpu_count
from datetime import datetime
from flask_wtf import FlaskForm
from wtforms import StringField, IntegerField, FloatField, SelectField, BooleanField, DateField, SubmitField
from wtforms.validators import DataRequired, NumberRange, Optional, ValidationError


OUTPUT_FOLDER = environ.get("OUTPUT_FOLDER", "output")
LANDMARK_MODEL = environ.get("LANDMARK_MODEL", "shape_predictor_68_face_landmarks.dat")


class TimelapseForm(FlaskForm):
    api_key = StringField(
        label="API Key",
        validators=[DataRequired()],
        default=environ.get("IMMICH_API_KEY", ""),
    )

    base_url = StringField(
        label="Base URL",
        validators=[DataRequired()],
        default=environ.get("IMMICH_BASE_URL", ""),
    )

    person_id = StringField(
        label="Person ID",
        validators=[DataRequired()],
        default=environ.get("DEFAULT_PERSON_ID", ""),
    )

    date_from = DateField(
        label="From Date",
        validators=[Optional()],
        default=datetime.strptime(environ.get("DEFAULT_DATE_FROM"), "%Y-%m-%d") if environ.get("DEFAULT_DATE_FROM") else None,
    )

    date_to = DateField(
        label="To Date",
        validators=[Optional()],
        default=datetime.strptime(environ.get("DEFAULT_DATE_TO"), "%Y-%m-%d") if environ.get("DEFAULT_DATE_TO") else None,
    )

    resize_size = IntegerField(
        label="Output Image Size",
        validators=[Optional(), NumberRange(min=1)],
        default=int(environ.get("DEFAULT_RESIZE_SIZE", 512)),
    )

    face_resolution_threshold = IntegerField(
        label="Minimum Face Resolution",
        validators=[Optional(), NumberRange(min=1)],
        default=int(environ.get("DEFAULT_FACE_RESOLUTION_THRESHOLD", 80)),
    )

    pose_threshold = FloatField(
        "Maximum Head Pose Deviation",
        validators=[Optional(), NumberRange(min=0)],
        default=float(environ.get("DEFAULT_POSE_THRESHOLD", 25.0)),
    )

    ear_threshold = FloatField(
        label="Minimum Eye Aspect Ratio",
        validators=[Optional(), NumberRange(min=0, max=1)],
        default=float(environ.get("DEFAULT_EAR_THRESHOLD", 0.2)),
    )

    max_workers = SelectField(
        "Max Workers (CPU cores)",
        coerce=int,
        choices=[(i, str(i)) for i in range(cpu_count() + 1)],
        default=int(environ.get("DEFAULT_MAX_WORKERS", 1)),
    )

    do_not_compile_video = BooleanField(
        label="Do not compile images into a video",
        default=environ.get("DEFAULT_DO_NOT_COMPILE_VIDEO", "").lower() in ("true", "1", "yes"),
    )

    framerate = IntegerField(
        label="Frames per second",
        validators=[Optional(), NumberRange(min=1)],
        default=int(environ.get("DEFAULT_FRAMERATE", 15)),
    )

    def validate_base_url(form, field):
        url = f"{field.data}/server/ping"
        headers = {
            'Accept': 'application/json',
        }
        try:
            response = requests.get(url, headers=headers, timeout=5)
            if response.status_code == 200:
                data = response.json()
                if data.get("res") == "pong":
                    return True
        except Exception as e:
            pass

        raise ValidationError("Invalid base URL")
    
    def validate_api_key(form, field):
        url = f"{form.base_url.data}/server/about"
        headers = {
            'Accept': 'application/json',
            'x-api-key': field.data,
        }
        try:
            response = requests.get(url, headers=headers, timeout=5)
            if response.status_code == 200:
                return True
        except Exception as e:
            pass

        raise ValidationError("Invalid API key")
