#!/usr/bin/env python3
"""Refresh placeholder dates in demo/*/data.json to local today and yesterday.

Usage:
    python3 demo/refresh_dates.py                 # refreshes demo/en/data.json
    python3 demo/refresh_dates.py path/to/data.json
"""

from datetime import datetime, timedelta
import json
from pathlib import Path
import sys

WEEKDAYS_EN = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
]


def refresh_file(path: Path) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    now = datetime.now().astimezone()
    today = now.date()
    yesterday = today - timedelta(days=1)

    today_str = today.isoformat()
    yesterday_str = yesterday.isoformat()
    today_ts = int(now.timestamp())
    yesterday_ts = int((now - timedelta(days=1)).timestamp())

    if "current_day" in data:
        data["current_day"]["date_iso"] = today_str

    profiles = data.get("profiles", [])
    if len(profiles) >= 1:
        profiles[0]["date_str"] = today_str
        profiles[0]["title"] = f"{WEEKDAYS_EN[today.weekday()]} — Today's Tasks"
        profiles[0]["created_ts"] = today_ts
    if len(profiles) >= 2:
        profiles[1]["date_str"] = yesterday_str
        profiles[1]["title"] = f"{WEEKDAYS_EN[yesterday.weekday()]} — Today's Tasks"
        profiles[1]["created_ts"] = yesterday_ts

    data["active_profile_idx"] = 0

    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Updated {path}: today={today_str}, yesterday={yesterday_str}")


def main() -> None:
    targets = [Path(arg) for arg in sys.argv[1:]] or [
        Path(__file__).resolve().parent / "en" / "data.json"
    ]
    for target in targets:
        refresh_file(target)


if __name__ == "__main__":
    main()
