# tck

`tck` is a small Rust terminal task app built around fast capture. It uses a three-pane TUI: group boxes across the top, a centered composer in the middle, and the active group's task log at the bottom. State is persisted to a single JSON file and saved after each meaningful change.

## Features

- `tck new <project-name>` creates `<project-name>.tck.json` and opens the TUI
- `tck open <path>` loads an existing project file and opens the TUI
- default `Inbox` group for new projects
- group selection, creation, and renaming
- group deletion with confirmation
- task capture from the center pane with an explicit typing mode
- task editing from the task pane
- task closing and reopening from the bottom pane
- copy selected task text to the system clipboard
- clear all closed tasks in the active group with confirmation from any non-writing mode
- bracketed-paste support for capture and group-name inputs
- lightweight terminal animations for focus, inserts, status flashes, and invalid actions

## Run

```bash
cargo run -- new demo
```

To reopen an existing project:

```bash
cargo run -- open demo.tck.json
```

## Controls

- `Up` / `Down`: move between panes, and also move through tasks while the task pane is focused
- `Left` / `Right`:
  - in groups: move across the top group boxes
  - in capture typing mode: move the text cursor
  - in tasks: `Left` moves focus back to the composer
- `1` / `2` / `3`: jump directly to groups, capture, or tasks
- `Enter`:
  - in capture pane: start a new capture
  - while typing a capture: save the capture to the active group
  - while editing a task: save the task text
  - in groups: activate the selected group
  - in tasks: toggle the selected task open/closed
  - in new-group / rename modals: confirm the name
- `n`: start creating a new group while groups pane is focused
- `r`: rename the selected group while groups pane is focused
- `x` while groups are focused: delete the selected group after confirmation
- `d`: clear all closed tasks in the active group after confirmation from any non-writing mode
- `e`: edit the selected task while tasks pane is focused
- `c`: copy the selected task text while tasks pane is focused
- `x`: close the selected task while tasks pane is focused
- `o` or `r`: reopen the selected task while tasks pane is focused
- `Esc`: cancel capture typing, task editing, renaming, new-group entry, or destructive confirmations
- `q` or `Ctrl-C`: quit

## Design notes

- JSON was chosen for persistence because it is easy to inspect and stable for a small MVP.
- Closed tasks remain visible with a crossed-out style and can be reopened in place. That keeps history visible without adding filtering yet.
- The updated layout adds group capsules, a blinking text cursor, task motion for add/close/reopen actions, and small ambient motion in the capture pane while keeping the screen restrained.
