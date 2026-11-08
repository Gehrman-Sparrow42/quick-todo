# Quick Todo

<!-- impeccable:product-schema 1 -->

## Platform

Native Windows desktop utility. This platform falls outside Impeccable's web, iOS and Android platform presets.

## Stack

Rust, as requested by the user. Native Win32 controls and an event-driven message loop keep the running application small and avoid an embedded browser.

## Users

One Windows user capturing ideas for their apps as they occur while using other applications.

## Product Purpose

A global keyboard shortcut opens a small writing window that appends ideas to an existing Obsidian Todo.md file. Capturing an idea should take only opening, typing and saving.

## Operating Context

The user confirmed automatic startup at Windows sign-in. Ctrl+Alt+T is the offered default shortcut. The target Markdown file already exists and contains plain text and bullets that must be preserved.

## Capabilities and Constraints

- Append Markdown tasks without rewriting existing notes.
- Use a compact native editor with a visible save action and keyboard access.
- Preserve unsaved text when hiding the window; retain it and show an error on a failed save.
- Keep all data local. No application runtime network access is needed.
- Use standard Windows utility conventions. Keep visual refinement modest: clear typography and spacing, a restrained Save accent and a recognizable app icon. Preserve the small runtime and event-driven idle behavior.
