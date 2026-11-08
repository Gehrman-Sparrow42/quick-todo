# Native capture window

Quick Todo uses standard Windows utility controls. This is a task surface: opening, writing and saving take precedence over decoration.

- Fixed 620 × 360 logical window, with Windows display scaling applied by the operating system.
- Segoe UI at 16 logical pixels, with a 24-pixel semibold heading.
- Quiet light surface (#f5f7f6), dark ink (#323629), muted helper text (#5b6053) and a deep teal primary action (#155f55).
- Twenty-eight-pixel content inset; a label directly above the native multiline edit control, with ten-pixel horizontal text margins.
- One idea per line; Save and Hide are visible beside the editor.
- Editor receives focus on opening, activation and the global shortcut. Tab reaches the native buttons.
- The close button and Escape hide the window without losing the in-memory draft.
- Save failures use the native error dialog and leave the draft in place.
- The application starts hidden at sign-in; no animation or periodic visual update runs while idle.
- A compact note/checkmark mark supplies the executable, taskbar, title bar and tray icon. The ICO contains 16, 20, 24, 32, 48, 64, 128 and 256-pixel variants.
- Native button semantics, pressed states and keyboard focus are preserved. Windows high-contrast settings use system colors and native button rendering at launch.

This Windows desktop platform is outside Impeccable's web/iOS/Android presets. Its native control semantics govern the interface.
