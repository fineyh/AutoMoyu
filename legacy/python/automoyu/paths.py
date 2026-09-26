"""集中管理文件路径。"""
from __future__ import annotations

import os

# .../AutoMoyu/legacy/python/automoyu/paths.py -> BASE_DIR = .../AutoMoyu/legacy/python
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# 数据仍放在仓库根目录的 data/（和 v0.1.0 一致，旧数据不用搬）
DATA_DIR = os.path.join(os.path.dirname(os.path.dirname(BASE_DIR)), "data")

CONFIG_PATH = os.path.join(DATA_DIR, "config.json")
STATS_PATH = os.path.join(DATA_DIR, "stats.json")


def ensure_data_dir() -> None:
    os.makedirs(DATA_DIR, exist_ok=True)
