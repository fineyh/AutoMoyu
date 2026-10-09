# 更新日志 / Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。
每个版本先写中文，`**English**` 下面是英文。

## [未发布]

## [1.0.0] - 2026-10-09

首个正式版本，稳定版更新通道自此版本起提供更新。本版本基于 Tauri 与 Rust 重新实现，不兼容 0.1.x 的配置。
以下内容汇总自 1.0.0-beta.1 至 1.0.0-beta.4；相较 1.0.0-beta.4，本版本无功能变更。

**钓鱼模式**

- 钓鱼机模式：配合半自动钓鱼机使用，由钓鱼机完成收竿，AutoMoyu 负责重新抛竿
- 野钓模式：无需钓鱼机，可在任意水域使用，检测到咬钩后自动收竿并重新抛竿

**识别与校准**

- 首次使用时自动校准（约 10 秒），校准结果按窗口分辨率保存，无需手动框选区域或调整阈值
- 基于手持鱼竿的状态进行识别，适用于白天、夜晚、雨天及昼夜过渡时段
- 野钓模式通过咬钩水花声判断咬钩，仅采集 Minecraft 进程的音频，并过滤猫叫等有音调的环境声

**兼容性**

- 支持高延迟服务器：可区分服务器回弹造成的抛竿回跳，避免重复抛竿
- 检测到 Windows 空间音效（Dolby Atmos 等）时，在首页给出提示及「声音设置」入口，并自动切换为采集系统整体音频
- 检测到玩家在游戏内操作鼠标或键盘时自动暂停，操作结束后自动恢复

**界面与系统**

- 统计页、系统托盘、游戏内浮层、自定义热键、自动停止条件及系统通知
- 自动更新，提供稳定版与测试版两个通道；更新提示内可直接展开完整更新说明
- 支持简体中文与英文界面

**English**

First stable release. The stable update channel begins with this version. AutoMoyu has been reimplemented with Tauri and Rust; configuration from 0.1.x is not compatible.
The notes below consolidate 1.0.0-beta.1 through 1.0.0-beta.4. There are no functional changes relative to 1.0.0-beta.4.

**Fishing modes**

- Fish farm: for use with a semi-automatic fish farm. The farm reels in and AutoMoyu recasts
- Open water: no fish farm required. Works on any body of water; AutoMoyu reels in when a bite is detected, then recasts

**Detection and calibration**

- Automatic calibration on first use (about 10 seconds), stored per window resolution. No manual region selection or threshold tuning is required
- Detection is based on the state of the held fishing rod and works in daylight, at night, in rain and during dusk and dawn transitions
- Open water mode detects bites from the splash sound, capturing audio from the Minecraft process only, and filters out tonal ambient sounds such as cat meows

**Compatibility**

- High-latency servers: cast bounce-backs caused by server correction are distinguished from real reel-ins, preventing duplicate casts
- When Windows spatial sound (Dolby Atmos etc.) is enabled, the home page displays a notice with a shortcut to Sound settings, and audio capture falls back to system-wide output
- Fishing pauses automatically while the player uses the mouse or keyboard in game and resumes once input stops

**Interface and system**

- Statistics page, system tray, in-game overlay, configurable hotkeys, auto-stop conditions and system notifications
- Automatic updates with stable and beta channels; full release notes can be expanded within the update notice
- Simplified Chinese and English interface

## [1.0.0-beta.4] - 2026-10-09

**变更**

- 更新提示中的「完整更新说明」改为在提示内直接展开，不再跳转至 GitHub；收起状态下按整行截断

**English**

**Changed**

- "Full release notes" now expands within the update notice instead of opening GitHub; collapsed notes are truncated at line boundaries

## [1.0.0-beta.3] - 2026-10-09

**变更**

- 两种模式更名为「钓鱼机模式」与「野钓模式」，按使用场景命名；移除野钓模式的「测试版」标识

**新增**

- 检测到 Windows 空间音效（Dolby Atmos 等）时，在首页给出提示及「声音设置」入口，并自动切换为采集系统整体音频，以保证咬钩检测正常工作

**修复**

- 猫叫等动物叫声被误判为咬钩的问题

**English**

**Changed**

- The two modes have been renamed "Fish farm" and "Open water" to reflect where they are used; the "beta" label has been removed from Open water mode

**Added**

- When Windows spatial sound (Dolby Atmos etc.) is enabled, the home page displays a notice with a shortcut to Sound settings, and audio capture falls back to system-wide output so that bite detection continues to work

**Fixed**

- Cat meows and other animal calls were incorrectly detected as bites

## [1.0.0-beta.2] - 2026-10-07

**新增**

- 更新提示中新增「完整更新说明」链接，用于查看因篇幅过长而被截断的更新说明全文

**English**

**Added**

- A "Full release notes" link in the update notice for viewing release notes that are too long to display in full

## [1.0.0-beta.1] - 2026-10-07

首个公开测试版本。本版本基于 Tauri 与 Rust 重新实现，不兼容 0.1.x 的配置。

- 提供两种模式：自动甩竿（配合钓鱼机使用）与全自动（测试功能），无需手动框选区域或调整灵敏度
- 首次使用时自动校准（约 10 秒），校准结果按窗口分辨率保存
- 基于手持鱼竿的状态进行识别，适用于白天、夜晚、雨天及昼夜过渡时段
- 全自动模式通过咬钩水花声判断咬钩，仅采集 Minecraft 进程的音频
- 支持高延迟服务器：可区分服务器回弹造成的抛竿回跳，避免重复抛竿
- 检测到玩家在游戏内操作鼠标或键盘时自动暂停，操作结束后自动恢复
- 统计页、系统托盘、游戏内浮层、自定义热键、自动停止条件及系统通知
- 自动更新，提供稳定版与测试版两个通道
- 支持简体中文与英文界面

**English**

First public pre-release. AutoMoyu has been reimplemented with Tauri and Rust; configuration from 0.1.x is not compatible.

- Two modes: auto-cast (for use with a fish farm) and full auto (experimental). No manual region selection or sensitivity tuning is required
- Automatic calibration on first use (about 10 seconds), stored per window resolution
- Detection is based on the state of the held fishing rod and works in daylight, at night, in rain and during dusk and dawn transitions
- Full auto mode detects bites from the splash sound, capturing audio from the Minecraft process only
- High-latency servers: cast bounce-backs caused by server correction are distinguished from real reel-ins, preventing duplicate casts
- Fishing pauses automatically while the player uses the mouse or keyboard in game and resumes once input stops
- Statistics page, system tray, in-game overlay, configurable hotkeys, auto-stop conditions and system notifications
- Automatic updates with stable and beta channels
- Simplified Chinese and English interface

## [0.1.0] - 2026-07-11

基于 Python 与 tkinter 的原型版本，源码位于 `legacy/python/`，未发布安装包。

**English**

Prototype built with Python and tkinter. The source is kept in `legacy/python/`; no installer was published.

[未发布]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.4...v1.0.0
[1.0.0-beta.4]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.3...v1.0.0-beta.4
[1.0.0-beta.3]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.2...v1.0.0-beta.3
[1.0.0-beta.2]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.1...v1.0.0-beta.2
[1.0.0-beta.1]: https://github.com/fineyh/AutoMoyu/compare/v0.1.0...v1.0.0-beta.1
[0.1.0]: https://github.com/fineyh/AutoMoyu/releases/tag/v0.1.0
