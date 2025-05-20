import requests
from os import environ
from multiprocessing import cpu_count
from datetime import datetime
from flask_wtf import FlaskForm
from wtforms import fields, validators


OUTPUT_FOLDER = "output"
LANDMARK_MODEL = "shape_predictor_68_face_landmarks.dat"


class TimelapseForm(FlaskForm):
    api_key = fields.StringField(
        label="API Key",
        validators=[validators.DataRequired()],
        default=environ.get("IMMICH_API_KEY", ""),
    )

    base_url = fields.StringField(
        label="Base URL",
        validators=[validators.DataRequired(), validators.URL()],
        default=environ.get("IMMICH_BASE_URL", ""),
    )

    person_id = fields.StringField(
        label="Person ID",
        validators=[validators.DataRequired()],
        default=environ.get("PERSON_ID", ""),
    )

    date_from = fields.DateField(
        label="From Date",
        validators=[validators.Optional()],
        default=datetime.strptime(environ.get("DATE_FROM"), "%Y-%m-%d") if environ.get("DATE_FROM") else None,
    )

    date_to = fields.DateField(
        label="To Date",
        validators=[validators.Optional()],
        default=datetime.strptime(environ.get("DATE_TO"), "%Y-%m-%d") if environ.get("DATE_TO") else None,
    )

    resize_size = fields.IntegerField(
        label="Output Image Size",
        validators=[validators.Optional(), validators.NumberRange(min=1)],
        default=int(environ.get("RESIZE_SIZE", 512)),
    )

    face_resolution_threshold = fields.IntegerField(
        label="Minimum Face Resolution",
        validators=[validators.Optional(), validators.NumberRange(min=1)],
        default=int(environ.get("FACE_RESOLUTION_THRESHOLD", 80)),
    )

    pose_threshold = fields.FloatField(
        "Maximum Head Pose Deviation",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=float(environ.get("POSE_THRESHOLD", 25.0)),
    )

    ear_threshold = fields.FloatField(
        label="Minimum Eye Aspect Ratio",
        validators=[validators.Optional(), validators.NumberRange(min=0, max=1)],
        default=float(environ.get("EAR_THRESHOLD", 0.2)),
    )

    max_workers = fields.SelectField(
        "Max Workers (CPU cores)",
        coerce=int,
        choices=[(i, str(i)) for i in range(cpu_count() + 1)],
        default=int(environ.get("MAX_WORKERS", 1)),
    )

    do_not_compile_video = fields.BooleanField(
        label="Do not compile images into a video",
        default=environ.get("DO_NOT_COMPILE_VIDEO", "").lower() in ("true", "1", "yes"),
    )

    framerate = fields.IntegerField(
        label="Frames per second",
        validators=[validators.Optional(), validators.NumberRange(min=1)],
        default=int(environ.get("FRAMERATE", 15)),
    )

    label = fields.BooleanField(
        label="Add date label",
        default=environ.get("LABEL", "").lower() in ("true", "1", "yes"),
    )
    label_size = fields.IntegerField(
        label="Label font size",
        validators=[validators.Optional(), validators.NumberRange(min=1)],
        default=int(environ.get("LABEL_SIZE", 12)),
    )
    label_pos = fields.SelectField(
        label="Label position",
        choices=[
            ("top_left", "Top Left"),
            ("top_right", "Top Right"),
            ("bottom_left", "Bottom Left"),
            ("bottom_right", "Bottom Right"),
        ],
        default=environ.get("LABEL_POS", "bottom_left"),
    )
    label_format = fields.StringField(
        label="Label date format",
        validators=[validators.Optional()],
        default=environ.get("LABEL_FORMAT", "%Y-%m-%d"),
    )
    label_padding = fields.IntegerField(
        label="Label padding",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=int(environ.get("LABEL_PADDING", 10)),
    )
    label_fill = fields.ColorField(
        label="Label fill color",
        validators=[validators.Optional()],
        default=environ.get("LABEL_FILL", "white"),
    )

    keep = fields.BooleanField(
        label="Filter images with a maxium of images per period",
        default=environ.get("KEEP", "").lower() in ("true", "1", "yes"),
    )
    keep_hourly = fields.IntegerField(
        label="Max per hour",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=int(environ.get("KEEP_HOURLY", 1)),
    )
    keep_daily = fields.IntegerField(
        label="Max per day",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=int(environ.get("KEEP_DAILY", 3)),
    )
    keep_weekly = fields.IntegerField(
        label="Max per week",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=int(environ.get("KEEP_WEEKLY", 10)),
    )
    keep_monthly = fields.IntegerField(
        label="Max per month",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=int(environ.get("KEEP_MONTHLY", 20)),
    )
    keep_yearly = fields.IntegerField(
        label="Max per year",
        validators=[validators.Optional(), validators.NumberRange(min=0)],
        default=int(environ.get("KEEP_YEARLY", 50)),
    )

    def validate_base_url(form, field):
        url = f"{field.data}/server/ping"
        headers = {
            'Accept': 'application/json',
        }
        try:
            response = requests.get(url, headers=headers, timeout=5)
            assert response.status_code == 200
        except Exception as e:
            raise validators.ValidationError("Invalid base URL")

    def validate_api_key(form, field):
        url = f"{form.base_url.data}/server/about"
        headers = {
            'Accept': 'application/json',
            'x-api-key': field.data,
        }
        try:
            response = requests.get(url, headers=headers, timeout=5)
            assert response.status_code == 200
        except Exception as e:
            raise validators.ValidationError("Invalid API key")

    def validate_person_id(form, field):
        url = f"{form.base_url.data}/people/{field.data}"
        headers = {
            'Accept': 'application/json',
            'x-api-key': form.api_key.data,
        }
        try:
            response = requests.get(url, headers=headers, timeout=5)
            assert response.status_code == 200
        except Exception as e:
            raise validators.ValidationError("Invalid person ID")

    def validate_label_format(form, field):
        try:
            datetime.now().strftime(field.data)
        except Exception:
            raise validators.ValidationError(
                "Invalid date format. Please refer to <a href='https://docs.python.org/3/library/datetime.html#strftime-and-strptime-format-codes'>strftime documentation</a>."
            )
