"""端到端冒烟：假游戏 + 真 AutoMoyu。

1. 清掉校准和设置（在 %APPDATA%\\AutoMoyu 下，先备份）
2. 起假游戏（钓鱼机 3 秒收竿）和 AutoMoyu（debug 版，需要 pnpm web 在跑）
3. 聚焦假游戏 → F6（没校准 → 打开向导）→ F6（开始校准）→ 等校准完 → F6（开始钓鱼）
4. 钓 25 秒 → F6 暂停 → 对比 AutoMoyu 统计库里的条数和假游戏报告的条数

只在没人用电脑的时候跑：会抢焦点、移动鼠标、按 F6。
"""
from __future__ import annotations

import ctypes
import os
import shutil
import sqlite3
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DATA = Path(os.environ["APPDATA"]) / "AutoMoyu"
u32 = ctypes.windll.user32
ctypes.windll.shcore.SetProcessDpiAwareness(2)

KEYEVENTF_KEYUP = 2
VK_F6 = 0x75


def find(title: str) -> int:
    return u32.FindWindowW(None, title)


def focus(h: int) -> None:
    u32.keybd_event(0x12, 0, 0, 0)  # Alt，解开前台锁
    u32.keybd_event(0x12, 0, KEYEVENTF_KEYUP, 0)
    u32.SetForegroundWindow(h)
    time.sleep(0.3)


def f6(game: int) -> None:
    if u32.GetForegroundWindow() != game:
        focus(game)
    assert u32.GetForegroundWindow() == game, "假游戏不在前台，放弃按键"
    u32.keybd_event(VK_F6, 0, 0, 0)
    time.sleep(0.05)
    u32.keybd_event(VK_F6, 0, KEYEVENTF_KEYUP, 0)
    time.sleep(0.3)


def main() -> int:
    backup = None
    if DATA.exists():
        backup = DATA.with_name("AutoMoyu.e2e-backup")
        if backup.exists():
            shutil.rmtree(backup)
        shutil.copytree(DATA, backup)
        for sub in ["calibration"]:
            shutil.rmtree(DATA / sub, ignore_errors=True)
        (DATA / "settings.json").unlink(missing_ok=True)

    game_p = subprocess.Popen([sys.executable, str(ROOT / "tools/fakegame/fake_minecraft.py"), "--machine", "3"],
                              stdout=subprocess.PIPE, text=True)
    time.sleep(1.5)
    app_p = subprocess.Popen([str(ROOT / "target/debug/automoyu.exe")], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(6)
    game = find("Minecraft")
    assert game, "假游戏窗口没出现"
    r = wintypes.RECT()
    u32.GetWindowRect(game, ctypes.byref(r))
    u32.SetCursorPos((r.left + r.right) // 2, (r.top + r.bottom) // 2)  # 右键要落在假游戏上
    try:
        # 首次运行会自动打开校准向导，这一下 F6 直接开始校准（3 秒倒计时 + 约 8 秒试甩）
        f6(game)
        time.sleep(15)
        cal = list((DATA / "calibration").glob("*.json"))
        print("校准文件：", [c.name for c in cal])
        assert cal, "校准没有生成结果"
        f6(game)  # 完成页 → 开始钓鱼
        time.sleep(25)
        f6(game)  # 暂停
        time.sleep(1)
    finally:
        app_p.terminate()
        game_p.terminate()
    out = game_p.stdout.read() if game_p.stdout else ""
    catches = out.count(" catch")
    db = sqlite3.connect(DATA / "stats.db")
    row = db.execute("SELECT id, catches, active_ms FROM sessions ORDER BY id DESC LIMIT 1").fetchone()
    casts = db.execute("SELECT COUNT(*) FROM events WHERE session_id=? AND kind='cast'", (row[0],)).fetchone()[0]
    print(f"假游戏：钓到 {catches} 条（全程含校准）；AutoMoyu 本场记录：{row[1]} 条，甩竿 {casts} 次，时长 {row[2]/1000:.0f}s")
    if backup:
        print(f"原数据备份在 {backup}")
    return 0 if row[1] >= 3 else 1


if __name__ == "__main__":
    sys.exit(main())
