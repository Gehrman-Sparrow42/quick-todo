# Native capture window

Quick Todo uses standard Windows utility controls. This is a task surface: opening, writing and saving take precedence over decoration.

- Fixed 620 × 360 logical window, with Windows display scaling applied by the operating system.
- Segoe UI at 16 logical pixels, with a 24-pixel semibold heading.
- Dark surface (#181c20), slightly lighter editor (#20262b), light ink (#e7edf0), muted helper text (#abb7be) and a deep teal primary action (#17695c).
- The native Windows title bar uses matching colors where DWM supports custom caption colors.
- Twenty-eight-pixel content inset; a label directly above the native multiline edit control, with ten-pixel horizontal text margins.
- One idea per line; Save and Hide are visible beside the editor.
- The editor label row shows Paste image and its Ctrl+V shortcut. An attached screenshot changes the label to Screenshot attached and offers Remove image.
- Editor receives focus on opening, activation and the global shortcut. Tab reaches the native buttons.
- The close button and Escape hide the window without losing the in-memory draft.
- Save failures use the native error dialog and leave the draft in place.
- The application starts hidden at sign-in; no animation or periodic visual update runs while idle.
- A compact note/checkmark mark supplies the executable, taskbar, title bar and tray icon. The ICO contains 16, 20, 24, 32, 48, 64, 128 and 256-pixel variants.
- Native button semantics, pressed states and keyboard focus are preserved. Windows high-contrast settings use system colors and native button rendering at launch.

This Windows desktop platform is outside Impeccable's web/iOS/Android presets. Its native control semantics govern the interface.
