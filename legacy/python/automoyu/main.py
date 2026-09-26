"""入口：设置 DPI 感知，启动 GUI。"""
from __future__ import annotations

import tkinter as tk

from . import winio
from .gui import App


def main() -> None:
    winio.set_dpi_awareness()
    root = tk.Tk()
    App(root)
    root.mainloop()


if __name__ == "__main__":
    main()
