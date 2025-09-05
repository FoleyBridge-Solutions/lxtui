# LXTUI Keyboard Shortcuts

## Main Container List

### Navigation
- **↑/↓** or **j/k** - Move selection up/down
- **Enter** - Open container actions menu for selected container
- **Space** - Open system menu

### Quick Container Actions (Direct from list)
- **s** - Start selected container
- **S** - Stop selected container  
- **d** - Delete selected container
- **n** - Create new container (wizard)
- **r/R** - Refresh container list

### Other
- **o/O** - Toggle operations sidebar
- **?/h** - Show help
- **q/Q** - Quit application
- **Ctrl+C** - Force quit

## Container Actions Menu (Enter on container)

Opens when you press **Enter** on a selected container:

- **Enter/s** - Smart action (Start if stopped, Stop if running)
- **1** - Start container
- **2** - Stop container
- **3** - Restart container
- **4** - Delete container
- **5** - Clone container
- **e** - Execute shell (container must be running)
- **Esc** - Close menu

## System Menu (Space)

Opens when you press **Space**:

- **1/r** - Refresh container list
- **2/l** - Check/start LXD service
- **3/n** - Create new container
- **4/o** - Toggle operations sidebar
- **5/h** - Show help
- **6/q** - Quit application
- **Esc** - Close menu

## Confirmation Dialogs

When confirming destructive actions:

- **Enter/y/Y** - Confirm action
- **Esc/n/N** - Cancel action

## Container Creation Wizard

- **Tab** - Next field
- **Shift+Tab** - Previous field
- **Enter** - Confirm on final step
- **Esc** - Cancel wizard

## Design Philosophy

The keybindings are designed to be intuitive and fast:

1. **Enter for context** - Pressing Enter on a container gives you all available actions
2. **Space for system** - System-wide operations are under Space
3. **Quick shortcuts** - Common actions (start/stop) have direct shortcuts
4. **Number keys in menus** - Menus support both numbers and mnemonics
5. **Vim-style navigation** - j/k for up/down movement
6. **Escape always cancels** - Consistent escape behavior across all menus

## Tips

- Use **s** and **S** for quick start/stop without opening menus
- The smart action (Enter in container menu) toggles between start/stop based on current state
- All menus show their keyboard shortcuts inline
- The bottom bar always shows available actions for current context