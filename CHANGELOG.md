# 更新日志 / Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。
每个版本先写中文，`**English**` 下面是英文。

## [未发布]

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

[未发布]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.2...HEAD
[1.0.0-beta.2]: https://github.com/fineyh/AutoMoyu/compare/v1.0.0-beta.1...v1.0.0-beta.2
[1.0.0-beta.1]: https://github.com/fineyh/AutoMoyu/compare/v0.1.0...v1.0.0-beta.1
[0.1.0]: https://github.com/fineyh/AutoMoyu/releases/tag/v0.1.0
