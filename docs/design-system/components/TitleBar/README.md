# TitleBar

The 40px top bar, which is also the window's drag region.

## Anatomy
- Height `size-titlebar`, `color-panel`, bottom hairline.
- macOS: the system draws the traffic lights over the bar; leave 80px on the left. Windows: Brachio draws minimize, maximize and close at the right edge, 46px wide each, after its own controls.
- Left: the repository switcher (folder icon, repository name at 600, chevron).
- Center: search, at most 460px wide, with its shortcut.
- Right: the Tasks and Board switch, notifications with the Needs you count, settings.
- Everything that is not a control drags the window.

## Don't
- Don't put a window title in the bar; the repository switcher names the context.
