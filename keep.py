import os
from datetime import datetime
from collections import defaultdict


def get_period_key(dt, period):
    if period == "hourly":
        return dt.strftime("%Y-%m-%d-%H")
    elif period == "daily":
        return dt.strftime("%Y-%m-%d")
    elif period == "weekly":
        return dt.strftime("%Y-W%U")
    elif period == "monthly":
        return dt.strftime("%Y-%m")
    elif period == "yearly":
        return dt.strftime("%Y")
    return None


def filter_assets_by_period(assets, config):
    """
    Filter assets to keep only a maximum per period (hour, day, week, month, year).
    """

    periods = [
        ("hourly", config.get("keep_hourly", 0)),
        ("daily", config.get("keep_daily", 0)),
        ("weekly", config.get("keep_weekly", 0)),
        ("monthly", config.get("keep_monthly", 0)),
        ("yearly", config.get("keep_yearly", 0)),
    ]

    filtered = assets
    filenames_to_remove = set()
    for period, max_per in periods:
        if max_per and max_per > 0:
            buckets = defaultdict(list)
            for asset in filtered:
                dt = datetime.fromisoformat(asset['fileCreatedAt'].replace("Z", "+00:00"))
                key = get_period_key(dt, period)
                buckets[key].append(asset)
            filtered = []
            for bucket in buckets.values():
                bucket_sorted = sorted(bucket, key=lambda a: a['_score'])
                filtered.extend(bucket_sorted[:max_per])
                filenames_to_remove.update(asset['_filename'] for asset in bucket_sorted[max_per:])

    for filename in filenames_to_remove:
        os.remove(filename)

    return filtered
