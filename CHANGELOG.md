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

- 更新提示里的「完整更新说明」改为直接在提示里展开，不再跳转 GitHub；收起时按整行截断

**English**

- "Full release notes" in the update notice now expands in place instead of opening GitHub; collapsed notes are cut at whole lines

## [1.0.0-beta.3] - 2026-10-09

- 模式改叫「钓鱼机模式」和「野钓模式」，按在哪钓来选；去掉了「测试版」标记
- 系统开了空间音效（Dolby Atmos 等）时，首页会提示并给出「声音设置」按钮；这时改录整机声音，咬钩照样能听到
- 猫叫等动物叫声不再被当成咬钩

**English**

- The modes are now called "Fish farm" and "Open water", named after where you fish; the "beta" tag is gone
- When Windows spatial sound (Dolby Atmos etc.) is on, the home page says so and offers a "Sound settings" button; AutoMoyu then records system-wide audio so bites are still heard
- Cats meowing and other animal calls no longer count as bites

## [1.0.0-beta.2] - 2026-10-07

- 更新提示里加了「完整更新说明」链接，说明太长被截断时可以点开看全文

**English**

- The update notice now has a "Full release notes" link for when the notes are too long to fit

## [1.0.0-beta.1] - 2026-10-07

第一个公开版本。用 Tauri + Rust 重写，和 0.1 不兼容，0.1 的设置不会迁移。

- 只需选模式：自动甩竿（有钓鱼机）或全自动（测试版）。不再需要框选区域、调灵敏度
- 首次使用自动校准，约 10 秒，按窗口尺寸保存
- 改为识别手持鱼竿的状态，白天、夜晚、雨天和昼夜交替都能认出
- 全自动模式改为听咬钩水花声，只录 Minecraft 自己的声音
- 能分辨高延迟服务器上的甩竿回跳，不会重复甩竿
- 你在游戏里操作时自动暂停，停手后继续
- 统计页、托盘、游戏内浮层、可改热键、自动停止条件、系统通知
- 自动更新，分稳定版和测试版两个通道
- 中文 / 英文界面

**English**

First public release. Rewritten with Tauri and Rust; settings from 0.1 are not carried over.

- Just pick a mode: auto-cast (with a fish farm) or full auto (beta). No more region picking or sensitivity tuning
- Calibrates itself on first use in about 10 seconds, saved per window size
- Detects the state of the rod in your hand; works by day, at night, in rain and through dusk and dawn
- Full auto now listens for the bite splash, using only Minecraft's own audio
- Recognises the cast jumping back on laggy servers and doesn't cast twice
- Pauses while you play and carries on when you stop
- Stats page, tray icon, in-game overlay, rebindable hotkeys, auto-stop conditions, system notifications
- Automatic updates with stable and beta channels
- English and Chinese interface

## [0.1.0] - 2026-07-11

Python + tkinter 原型，代码在 `legacy/python/`，没有发布安装包。

**English**

Python + tkinter prototype, kept in `legacy/python/`. No installer was published.

[未发布]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.4...v1.0.0
[1.0.0-beta.4]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.3...v1.0.0-beta.4
[1.0.0-beta.3]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.2...v1.0.0-beta.3
[1.0.0-beta.2]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.1...v1.0.0-beta.2
[1.0.0-beta.1]: https://github.com/fineyh/AutoMoyu/compare/v0.1.0...v1.0.0-beta.1
[0.1.0]: https://github.com/fineyh/AutoMoyu/releases/tag/v0.1.0
