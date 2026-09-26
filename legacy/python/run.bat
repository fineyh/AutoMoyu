@echo off
chcp 65001 >nul
cd /d "%~dp0"
rem 用 pythonw 静默启动（无黑框）。若想看报错，把下面一行改成 python run.py
start "" pythonw run.py
