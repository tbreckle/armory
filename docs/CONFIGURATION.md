# Configuration

A.R.M.O.R.Y. is configured with a single YAML file, `armory.yaml`. A complete example is in
[`examples/armory.yaml`](../examples/armory.yaml).

## Where the file is found

1. The file given with `--config <file>` (or `-c <file>`)
2. `armory.yaml` next to the executable (portable setups)
3. `armory/armory.yaml` in the user's config directory:
   - Linux: `~/.config/armory/armory.yaml`
   - Windows: `%APPDATA%\armory\armory.yaml`
   - macOS: `~/Library/Application Support/armory/armory.yaml`

If there is no file or it has an error, ARMORY shows an error screen with the exact line and
large **Reload** and **Quit** buttons. Fix the file, then shoot **Reload**.

Relative paths (images, programs, `cwd`) are relative to the folder of the YAML file.

## Structure

```yaml
version: 1          # optional, format version
style: { ... }      # optional, look and sizes
status_texts: { ... } # optional, global texts of the status states
tabs:               # required, at least one tab
  - name: "Games"
    image: images/games.png   # optional icon on the tab
    items:                    # buttons, labels and dividers, top to bottom
      - divider: "Section"
      - button: { ... }
      - label: ...
```

Unknown keys are errors, so typos don't go unnoticed.

## `style`

All values are optional; sizes are logical pixels (scaled with the display scaling).

| Key | Default | Meaning |
|---|---|---|
| `theme` | `dark` | `dark` or `light` |
| `font_size` | `28` | Base font size |
| `tab_width` | `340` | Width of the tab column |
| `tab_height` | `110` | Height of a tab |
| `button_width` | `300` | Width of buttons and tile labels |
| `button_height` | `200` | Height of buttons and tile labels |
| `spacing` | `20` | Gap between items |
| `accent_color` | `"#ffb400"` | Active tab and separator |
| `status_colors` | on `"#2ecc71"`, off `"#8a8f98"`, fault `"#e74c3c"` | Colors of the status states; set only the ones you want to change |

## Items

Tabs are shown as large buttons in a column on the left. Inside a tab, buttons (and tile
labels) wrap into rows at a fixed size; dividers and normal labels take a full row. If a tab
doesn't fit, large page up/down buttons appear on the right; the same happens in the tab
column when there are more tabs than fit.

### Divider

A horizontal line, optionally with a centered label.

```yaml
- divider:                 # plain line
- divider: "Sega Model 3"  # line with label
```

### Button

A button shows an image, a label, or both (at least one is required). With both, the image
is centered on top and the label at the bottom; a single one is centered.

```yaml
- button:
    label: "The Lost World"
    image: images/lostworld.png
    run: C:/Emulators/Supermodel/Supermodel.exe
    args: ["roms/lostwsga.zip", "-fullscreen"]
```

| Key | Meaning |
|---|---|
| `label` | Text of the button |
| `image` | Image file (PNG, JPEG, SVG, GIF, BMP, WebP, ...) |
| `run` | Program to start, see below |
| `args` | List of arguments (not possible for files opened through their file association) |
| `cwd` | Working directory. Default: the folder of `run` |
| `with` | Program that opens `run`, e.g. `with: C:/Program Files/AutoHotkey/v2/AutoHotkey64.exe`. Overrides the automatic detection |
| `on_launch` | `stay` (default): ARMORY stays open. `minimize`: ARMORY minimizes and comes back when the program ends |
| `wait_for` | With `on_launch: minimize`: process name to wait for instead of the started process, see below |
| `action` | Instead of `run`: a built-in function, `quit`, `reload` (re-read the YAML file) or `minimize` |

**What `run` starts:**

| `run` | Windows | Linux | macOS |
|---|---|---|---|
| Executable (`.exe`, `.com`; files with the executable bit) | directly | directly | directly |
| Script | `.bat`/`.cmd` directly, `.ps1` with PowerShell | `.sh` with `sh`, `.bash` with `bash` | `.sh`/`.command` with `sh`, `.bash` with `bash` |
| App bundle (`.app`) | – | – | `open` |
| Any other file (`.ahk`, `.pdf`, ...) | file association (`start`) | file association (`xdg-open`) | file association (`open`) |
| Bare name without folder (`mame`) | from `PATH` | from `PATH` | from `PATH` |

A bare name is only looked up in `PATH` if no file with that name exists next to the YAML file.

**`on_launch: minimize` and `wait_for`:** by default, ARMORY comes back when the started process
ends. Some programs hand over to another process and exit right away (launchers, and files opened
through their file association on Windows and Linux). Then set `wait_for` to the process name of
the real program: ARMORY comes back once that process has started and ended again (or after
30 seconds if it never appears). For files opened through their file association, `wait_for` is
required with `on_launch: minimize`.

### Label

Text only. Either just the text, or a map with more settings:

```yaml
- label: "Just some text"
- label:
    text: "Sinden driver: {STATUS}"
    monitor: SindenLightgun.exe
    status_texts: { on: "Ready" }
    tile: true
```

| Key | Meaning |
|---|---|
| `text` | The text. `{STATUS}` is replaced by the status text; `{{` and `}}` write literal braces |
| `monitor` | Process name to watch; the label then shows its status |
| `status_texts` | Texts of the states for this label (`on`, `off`, `fault`); only the given ones change |
| `tile` | `true`: button-sized tile among the buttons. Default: a full-width row |

## Status of monitored processes

ARMORY checks all monitored processes once per second. The status always has one of three
states:

| State | Meaning | Default text |
|---|---|---|
| `on` | exactly one process with that name runs | `On` |
| `off` | no process with that name runs | `Off` |
| `fault` | more than one runs, or the process list couldn't be read | `Fault` |

A monitored label shows a dot and a border in the state's color (`style.status_colors`).

**Process names** are compared without folder and without `.exe`, so `SindenLightgun.exe`
and `SindenLightgun` are the same. On Windows and macOS, case doesn't matter. A process matches
by its process name, its executable file name or the first entry of its command line, so
Windows programs under Wine match on Linux as well.

**Status texts** are chosen in this order: the label's `status_texts`, then the global
`status_texts`, then the defaults. Each state is looked up separately, so you can rename only
some of them:

```yaml
status_texts:
  on: "Yes"               # global: on → Yes, off and fault keep "Off" and "Fault"
tabs:
  - name: Tools
    items:
      - label:
          text: "Recoil: {STATUS}"
          monitor: AutoHotkey64.exe
          status_texts: { off: "stopped" }   # here: Yes / stopped / Fault
```
