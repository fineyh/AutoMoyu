"""集中管理文件路径。"""
from __future__ import annotations

import os

# .../AutoMoyu/automoyu/paths.py -> BASE_DIR = .../AutoMoyu
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_DIR = os.path.join(BASE_DIR, "data")

CONFIG_PATH = os.path.join(DATA_DIR, "config.json")
STATS_PATH = os.path.join(DATA_DIR, "stats.json")


def ensure_data_dir() -> None:
    os.makedirs(DATA_DIR, exist_ok=True)
