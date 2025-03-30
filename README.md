<p align="center" style="background-color: #57887e; width: 210px; height: 210px; border-radius: 50%; display: flex; justify-content: center; align-items: center; margin: auto;">
  <img src="./assets/randolf.png" width="200" height="200" alt="Randolf" style="border-radius: 50%;"/>
</p>

# Meet Randolf

> [!NOTE]
> This project is still under active development and lacks important features.

Randolf is a partial window manager for Windows 11. Randolf allows you to:

- `Win` + `\` - near-maximise the active window (maximise minus margin).
- `Win` + `Shift` + `Left`/`Top`/`Right`/`Down` or `h`/`j`/`k`/`l` - near-snap (snap minus margin) the active window
  to the left, top, right, or bottom of the screen.
- `Win` + `Left`/`Top`/`Right`/`Down` - move the cursor to the closest window in the direction of the arrow key (and
  highlight the window) or to the center of the window-free monitor, if it exists.
- `Win` + `q` - close the active window.

My goal for this project was to implement some key window navigation concepts
from [my Linux configuration](https://github.com/kimgoetzke/nixos-config) for Windows, offering an experience,
somewhat closer to that of Linux window managers/compositors such as [Hyprland](https://hyprland.org/). The
application was created to meet my own needs and started as migration of [Randy](https://github.com/kimgoetzke/randy)
from C#/.NET to Rust, however contributions or suggestions are welcome.

#### Additional features

- Pressing `Win` + `\` on a near-maximised window will reset the window to its previous size and position (i.e. undo the
  near-maximisation)
- Writes application logs to a file in the directory of the executable (can be disabled)
- Stores and loads configuration from `randolf.toml` in the directory of the executable
- Tray icon with a context menu
    - Allows customising the window margin
    - Allows closing the application

#### Features under consideration

- Allow auto-start application on startup
- Allow customising hotkeys
- Group "snapped" window on a single screen and allow resizing them together
- Add virtual desktops navigation

## How to configure

The configuration file `randolf.toml` is located in the same directory as the executable after the first start. The
configuration file is created with the following default values:

```toml
file_logging_enabled = false
default_margin = 20
```

## How to develop

### Prerequisites

You'll need the C++ tools from the Build Tools for Visual Studio installed.

Useful links:

- [Programming reference for the Win32 API](https://learn.microsoft.com/en-us/windows/win32/api/)