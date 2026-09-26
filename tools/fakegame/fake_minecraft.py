"""假的 "Minecraft" 窗口，用来在没有游戏的机器上端到端跑 AutoMoyu。

- 窗口标题就叫 Minecraft，AutoMoyu 会把它当成游戏。
- 右下角画一根"手持鱼竿"：收回 / 甩出两种样子；背景是不停晃动的水面噪声。
- 右键：收回 → 甩出（甩竿），甩出 → 收回（收竿）。
- --machine N：甩出 N 秒后自动收回，模拟钓鱼机（每次算钓到一条）。

用法：python tools/fakegame/fake_minecraft.py --machine 3
每次状态变化会在标准输出打一行，方便脚本统计。
"""
from __future__ import annotations

import argparse
import random
import sys
import time
import tkinter as tk

W, H = 800, 450


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--machine", type=float, default=3.0, help="甩出多少秒后自动收回；0 = 不自动")
    args = ap.parse_args()
    try:  # 和真游戏一样按物理像素渲染
        import ctypes
        ctypes.windll.shcore.SetProcessDpiAwareness(2)
    except Exception:
        pass

    root = tk.Tk()
    root.title("Minecraft")
    root.geometry(f"{W}x{H}+200+150")
    root.resizable(False, False)
    c = tk.Canvas(root, width=W, height=H, highlightthickness=0, bg="#3a6fd8")
    c.pack()

    # 水面：一堆会动的横线
    waves = [c.create_line(0, 0, 0, 0, fill="#8fb4ff", width=3) for _ in range(40)]
    sky = c.create_rectangle(0, 0, W, H * 0.4, fill="#8cc4ff", outline="")
    state = {"out": False, "since": 0.0, "casts": 0, "catches": 0}
    rod: list[int] = []

    def draw_rod() -> None:
        for r in rod:
            c.delete(r)
        rod.clear()
        x0, y0 = W * 0.78, H * 0.62
        if state["out"]:
            # 甩出：竿抬高，挂一根线
            rod.append(c.create_line(x0 - 40, y0 - 70, W * 0.95, H, fill="#5b3a1e", width=14))
            rod.append(c.create_line(x0 - 40, y0 - 70, W * 0.5, H * 0.45, fill="#e8e8e8", width=2))
        else:
            rod.append(c.create_line(x0, y0, W * 0.95, H, fill="#5b3a1e", width=14))
            rod.append(c.create_rectangle(x0 - 6, y0 - 6, x0 + 6, y0 + 6, fill="#b0b0b0", outline=""))
        # 快捷栏
        rod.append(c.create_rectangle(W * 0.3, H * 0.9, W * 0.7, H * 0.98, fill="#333", outline="#999"))

    def log(msg: str) -> None:
        print(f"{time.time():.3f} {msg}", flush=True)

    def toggle(_e=None) -> None:
        state["out"] = not state["out"]
        state["since"] = time.time()
        if state["out"]:
            state["casts"] += 1
            log("cast")
        else:
            log("reel")
        draw_rod()

    def tick() -> None:
        t = time.time()
        for i, w in enumerate(waves):
            y = H * 0.42 + (i * 23 % int(H * 0.55))
            x = (i * 97 + t * (20 + i % 7) * 4) % (W + 200) - 100
            c.coords(w, x, y + random.uniform(-2, 2), x + 60 + i % 5 * 10, y)
        c.itemconfig(sky, fill="#8cc4ff")
        if state["out"] and args.machine > 0 and t - state["since"] >= args.machine:
            state["out"] = False
            state["since"] = t
            state["catches"] += 1
            log("catch")
            draw_rod()
        root.after(33, tick)

    c.bind("<Button-3>", toggle)
    draw_rod()
    tick()
    log("ready")
    root.mainloop()
    sys.exit(0)


if __name__ == "__main__":
    main()
