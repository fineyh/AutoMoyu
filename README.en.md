# AutoMoyu

[![Release](https://img.shields.io/github/v/release/fineyh/AutoMoyu?include_prereleases&label=release)](https://github.com/fineyh/AutoMoyu/releases)
[![CI](https://github.com/fineyh/AutoMoyu/actions/workflows/ci.yml/badge.svg)](https://github.com/fineyh/AutoMoyu/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[中文](README.md) | English

An auto-fishing tool for Minecraft Bedrock Edition on Windows. AutoMoyu monitors the state of the held fishing rod and recasts automatically once the rod has been reeled in. Without a fish farm, it can also detect bites by sound and fish fully automatically.

<p>
  <img src="assets/screenshots/home-en.jpg" width="300" alt="Home screen while fishing">
  <img src="assets/screenshots/stats-en.jpg" width="300" alt="Stats screen">
</p>

## Features

- **Two modes**, both fully automatic, chosen by where you fish:
  - Fish farm: for use with an in-game semi-automatic fish farm. The farm reels in and AutoMoyu recasts.
  - Open water: no fish farm required. Works on any body of water; AutoMoyu reels in when a bite is detected, then recasts.
- **No manual setup**: no region selection or threshold tuning. Calibration runs automatically on first use (about 10 seconds) and is needed only once per window size.
- Works in daylight, at night and in rain, and adapts to changing brightness during dusk and dawn.
- High-latency servers: cast bounce-backs caused by server correction are distinguished from real reel-ins, preventing duplicate casts.
- Yields control while the player uses the mouse or keyboard in game, and resumes 5 seconds after input stops.
- Stops automatically and reports the reason when a cast fails, the rod cannot be recognised, or the game exits.
- Statistics for daily catches and average time to bite.
- System tray, in-game overlay and configurable hotkeys (F6 to start/pause and F7 to toggle the overlay by default).
- Automatic updates; Simplified Chinese and English interface.

## Requirements

- Windows 10 version 2004 or later, 64-bit
- Minecraft Bedrock Edition (Microsoft Store or Xbox app)
- The game must run in **windowed or borderless** mode; exclusive fullscreen cannot be captured

## Installation

Download `AutoMoyu_<version>_x64-setup.exe` from [Releases](https://github.com/fineyh/AutoMoyu/releases) and run it. AutoMoyu installs for the current user and does not require administrator rights. If WebView2 is not present, the installer downloads it automatically (it is included with Windows 11).

The installer is not yet code-signed, so Windows SmartScreen may block it. Click **More info**, then **Run anyway**.

AutoMoyu checks for updates automatically. No notice is shown while fishing; once fishing stops, an update notice appears on the Home screen.

## Usage

1. Start the game and hold a fishing rod. In Fish farm mode, stand at the farm's fishing position. In Open water mode, aim the crosshair at the water.
2. Calibration opens on first launch. Switch to the game and press **F6**. AutoMoyu casts twice on its own; do not move the mouse until calibration completes.
3. Press **F6** to start fishing and press it again to pause. **F7** shows or hides the in-game overlay.

The game window must stay in the foreground while AutoMoyu is running. Switching to another window pauses fishing, and switching back resumes it. If the window size changes, you will be prompted to recalibrate.

Recommended settings for reliable detection:

- Turn off View Bobbing in the game settings and do not use shaders.
- Open water mode detects bites by sound, so keep the game's sound effects volume above 0. Game music can be turned off; music played on the computer and voice chat do not interfere.
- Turn off Windows spatial sound (Dolby Atmos, Windows Sonic, etc.). While it is on, AutoMoyu has to capture system-wide audio instead, and sounds from other applications may cause false reel-ins.

## FAQ

**Minecraft isn't found**

Make sure the game is in windowed or borderless mode. If it is still not detected, select it manually under Settings → Advanced → Game window.

**"The cast didn't go out"**

This usually means the item in hand is not a fishing rod or there is no water in front of the player. Resolve the issue and press F6 to continue.

**"Can't recognise the rod"**

This usually means the inventory or a menu is open. Close it and fishing resumes automatically. If it happens frequently, recalibrate under Settings → Fishing.

**"Spatial sound is on"**

In Windows Settings → System → Sound, open the output device's properties and set Spatial sound to Off. The "Sound settings" button in the notice opens that page directly. The notice disappears once spatial sound is off.

**"High server lag makes the cast jump back; adjusted automatically"**

No action is needed. The notice is informational only, and AutoMoyu continues waiting for bites as usual.

**Is there a risk of being banned?**

AutoMoyu only sends right-clicks to the game window. It does not read or write game memory, modify game files or inject any code. However, many servers prohibit AFK fishing. **Check your server's rules before use**; you use AutoMoyu at your own risk.

**How do I report a problem?**

Export a diagnostics package under Settings → Advanced, then [open an issue](https://github.com/fineyh/AutoMoyu/issues/new/choose) and attach the zip file. Opening an issue from Settings → About → Report a problem fills in the version and window information automatically.

## How it works

- **Rod state detection**: during calibration AutoMoyu casts twice and records the game screen with the rod reeled in and cast out. It then selects the area near the held rod or the hotbar where the two states differ most. While running, it captures that area 15 times per second and determines which state the current frame is closer to.
- **Bite detection** (Open water mode): audio is captured from the Minecraft process only, using WASAPI process loopback. A bite splash appears as a sustained burst of broadband noise; AutoMoyu evaluates its loudness and duration, so the brief sounds of an approaching fish are not mistaken for a bite. Tonal sounds such as cat meows are also excluded.
- **Input simulation**: right-clicks are simulated through Windows `SendInput`, and by default only while the game is the foreground window.

All processing takes place locally. The only network access is the update check.

## Contributing

Build instructions, project layout and the offline evaluation tools are documented in [CONTRIBUTING.md](CONTRIBUTING.md) (in Chinese; the commands work as written). Release history is in [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE)

Not affiliated with Mojang or Microsoft. Minecraft is a trademark of Mojang Synergies AB.
