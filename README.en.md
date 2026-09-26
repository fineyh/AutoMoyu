# AutoMoyu 🎣 — auto fishing for Minecraft Bedrock

[中文](README.md) | **English**

**Pick a mode, calibrate once, go do something else.**

You answer one question: do you have a fish farm? AutoMoyu casts twice by itself to find the clearest part of the screen and sets its own thresholds. Then press F6 and leave it.
Failed casts are retried, it pauses when you switch away from the game and resumes when you come back, and when it stops it always tells you why and what to do.

| Mode | For | What AutoMoyu does |
|---|---|---|
| **Auto-cast** (recommended) | You have a semi-automatic fish farm that reels in on a bite | Casts again whenever the rod comes back |
| **Full auto** (beta) | No farm, just facing water | Reels in when it hears the bite splash, then casts again |

## Install

Download `AutoMoyu_x.y.z_x64-setup.exe` from [Releases](https://github.com/fineyh/AutoMoyu/releases) and run it (no admin rights needed).
The installer isn't code-signed yet, so Windows may say "unknown publisher": click **More info → Run anyway**.

AutoMoyu checks for updates by itself. It never interrupts a running session; once you stop, the Home page offers "Restart & update".

The interface follows your Windows display language (Chinese or English). You can change it under Settings → General → Language.

## Use

1. Start Minecraft Bedrock Edition in **windowed or borderless** mode (exclusive fullscreen can't be captured).
2. Hold a fishing rod and stand at your fish farm (full auto: aim at water).
3. On first launch AutoMoyu opens calibration: switch to the game and press **F6**. It casts twice by itself, which takes about 10 seconds.
4. After calibration press **F6** to start fishing, and again to pause. **F7** shows or hides the in-game overlay.

If the window size changes you'll be asked to recalibrate (once per size).

**Tips**: turn off View Bobbing and shaders for a steadier signal. In full auto, keep the game's sound effects volume above 0 (music can be off).

## FAQ

- **Minecraft not found**: make sure the game is windowed or borderless. If it still isn't found, pick it under Settings → Advanced → Game window.
- **The cast didn't go out**: you may not be holding a fishing rod, or there's no water ahead. Fix it, then press F6.
- **Can't recognise the rod**: usually a menu or inventory is open. Close it and AutoMoyu resumes by itself. If it keeps happening, recalibrate.
- **"High server lag makes the cast jump back"**: on distant or laggy servers the game shows the cast, the server pulls it back, and then it goes out again. AutoMoyu recognises this and keeps waiting for a fish; nothing to do.
- **Reporting a problem**: Settings → Advanced → Export diagnostics, then attach the zip to an [issue](https://github.com/fineyh/AutoMoyu/issues).

> AutoMoyu only simulates right-clicks in the game window. It doesn't read or write game memory, doesn't change game files, and doesn't upload anything (it only checks for updates).
> AFK fishing may break some servers' rules. **Check that your server allows it; you use AutoMoyu at your own risk.**
> This project is not affiliated with Mojang or Microsoft and is not an official Minecraft product.

## License

[MIT](LICENSE)

Development notes are in the [Chinese README](README.md#开发).
