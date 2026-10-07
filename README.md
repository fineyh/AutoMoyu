# AutoMoyu

[![Release](https://img.shields.io/github/v/release/fineyh/AutoMoyu?include_prereleases&label=release)](https://github.com/fineyh/AutoMoyu/releases)
[![CI](https://github.com/fineyh/AutoMoyu/actions/workflows/ci.yml/badge.svg)](https://github.com/fineyh/AutoMoyu/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

中文 | [English](README.en.md)

Minecraft 基岩版（Windows）的自动钓鱼工具。它盯着你手里的鱼竿，竿收回来了就再甩出去；没有钓鱼机也能全自动钓鱼。

<p>
  <img src="assets/screenshots/home-zh.jpg" width="300" alt="首页：正在钓鱼">
  <img src="assets/screenshots/stats-zh.jpg" width="300" alt="统计页">
</p>

## 功能

- **两种模式**
  - 自动甩竿：配合游戏里的半自动钓鱼机用。机器负责收竿，AutoMoyu 负责再甩。
  - 全自动（测试版）：不需要钓鱼机，对着水面钓，听到咬钩就收竿，然后再甩。
- **不用框选、不用调参数**。第一次使用时自动校准，大约 10 秒，同一个窗口尺寸只需校准一次。
- 白天、夜晚、下雨都能认出鱼竿，昼夜交替时会跟着画面亮度自己调整。
- 在高延迟服务器上，甩出去的竿会被服务器拉回来再甩出去，AutoMoyu 能分辨这种回跳，不会多甩。
- 你在游戏里动鼠标或按键时自动让出控制，停手 5 秒后接着钓。
- 没甩出去、认不出鱼竿、游戏关了，都会停下来告诉你原因。
- 统计每天钓了多少条、平均多久上钩。
- 托盘图标、游戏内浮层、热键可改（默认 F6 开始/暂停，F7 显示浮层）。
- 自动更新；中文和英文界面。

## 系统要求

- Windows 10 2004 或更新版本（64 位）
- Minecraft 基岩版，Microsoft Store 或 Xbox App 安装的都可以
- 游戏设为**窗口化或无边框**，独占全屏下截不到画面

## 安装

到 [Releases](https://github.com/fineyh/AutoMoyu/releases) 下载 `AutoMoyu_<版本>_x64-setup.exe` 运行即可，装在当前用户目录下，不需要管理员权限。系统里没有 WebView2 的话，安装程序会自动下载（Windows 11 自带）。

安装包暂时没有代码签名，Windows SmartScreen 可能拦一下，点「更多信息」→「仍要运行」。

AutoMoyu 会自己检查更新。钓鱼的时候不会弹，停下来后首页会出现更新提示。

## 使用

1. 打开游戏，手持鱼竿。有钓鱼机就站到机器的位置；全自动模式就把准星对准水面。
2. 第一次打开 AutoMoyu 会进入校准。切回游戏按 **F6**，程序会自己甩两次竿，校准期间别动鼠标。
3. 校准完成后按 **F6** 开始，再按一次暂停。**F7** 显示或隐藏游戏内浮层。

挂机时游戏窗口要保持在前台。切到别的窗口会自动暂停，切回来自动继续。窗口尺寸变了会提示重新校准。

几个能让识别更稳的设置：

- 关掉游戏里的「视角摇晃」，不要开光影
- 全自动模式要靠声音判断，「音效」音量别调到 0；音乐可以关，电脑上放歌、语音聊天不影响

## 常见问题

**找不到 Minecraft**

确认游戏是窗口化或无边框。还不行的话，在 设置 → 高级 → 游戏窗口 里手动选。

**提示没甩出去**

多半是手上拿的不是鱼竿，或者面前没有水。处理好后按 F6 继续。

**提示认不出鱼竿**

通常是开了背包或菜单，关掉后会自动继续。经常出现的话，在 设置 → 钓鱼 里重新校准。

**提示「服务器延迟高，甩竿画面会回跳，已自动适应」**

不用处理，只是告诉你一声，程序会照常等鱼上钩。

**会不会被封号？**

AutoMoyu 只往游戏窗口发鼠标右键，不读写游戏内存，不改游戏文件，也不注入任何东西。但很多服务器禁止挂机钓鱼，**用之前请先看服务器规则**，因此造成的后果由使用者自己承担。

**遇到问题怎么反馈？**

在 设置 → 高级 → 导出诊断包，然后到 [Issues](https://github.com/fineyh/AutoMoyu/issues/new/choose) 开一个问题，把 zip 拖进去。设置 → 关于 → 反馈问题 会自动填好版本和窗口信息。

## 工作原理

- **判断鱼竿状态**：校准时程序自己甩两次竿，分别记下「竿收回」和「竿甩出」时游戏画面的样子，然后在手持鱼竿和快捷栏附近挑一块两种状态差别最大的区域。运行时每秒截这块区域 15 次，看当前画面离哪种状态更近。
- **听咬钩**（全自动模式）：只录 Minecraft 进程自己的声音（WASAPI 进程环回），咬钩时的水花声是一段持续的宽频噪声，程序根据它的响度和持续时间来判断，鱼游过来时那种短促的小声响不会被当成咬钩。
- **点击**：用 Windows 的 `SendInput` 模拟鼠标右键，默认只在游戏处于前台时才点。

全程只在本机运行，除了检查更新不联网。

## 参与开发

构建方法、目录结构和离线评估工具见 [CONTRIBUTING.md](CONTRIBUTING.md)，版本变化见 [CHANGELOG.md](CHANGELOG.md)。

## 许可证

[MIT](LICENSE)

本项目与 Mojang、Microsoft 无关。Minecraft 是 Mojang Synergies AB 的商标。
