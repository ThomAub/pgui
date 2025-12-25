# Keyboard Navigation Design

## Overview

This document describes the keyboard navigation system for PGUI, providing power-user keyboard shortcuts throughout the application.

## Design Principles

1. **Vim-style + Standard**: Support both vim keybindings (j/k) and standard arrow keys
2. **Context-aware**: Different shortcuts active based on focused panel
3. **Discoverable**: Help overlay with `?` key showing all available shortcuts
4. **Modal**: Global shortcuts work everywhere, local shortcuts work in specific contexts

## Architecture

```
src/
├── keybindings/
│   ├── mod.rs          # Module exports, keymap registration
│   ├── actions.rs      # All Action type definitions
│   └── bindings.rs     # Default keymap definitions
```

## Keymap Design

### Global Shortcuts (Work Everywhere)

| Key | Action | Description |
|-----|--------|-------------|
| `Cmd+1` | SwitchToDatabase | Switch to Database mode |
| `Cmd+2` | SwitchToStorage | Switch to Storage mode |
| `Cmd+B` | ToggleSidebar | Toggle left sidebar (tables/connections) |
| `Cmd+\` | ToggleRightPanel | Toggle right panel (agent/history) |
| `Escape` | Unfocus / Cancel | Clear focus or cancel current action |
| `?` | ShowHelp | Show keyboard shortcuts help overlay |
| `Cmd+Q` | Quit | Quit application |

### Database Mode - Disconnected (Connection List)

| Key | Action | Description |
|-----|--------|-------------|
| `j` / `Down` | NextConnection | Move to next connection |
| `k` / `Up` | PrevConnection | Move to previous connection |
| `Enter` | Connect | Connect to selected connection |
| `e` | EditConnection | Edit selected connection |
| `d` | DeleteConnection | Delete selected connection |
| `n` | NewConnection | Create new connection |

### Database Mode - Connected

| Key | Action | Description |
|-----|--------|-------------|
| `Cmd+Enter` | ExecuteQuery | Execute current query |
| `Cmd+E` | FocusEditor | Focus SQL editor |
| `Cmd+T` | FocusTables | Focus tables tree |
| `Cmd+R` | FocusResults | Focus results panel |
| `Cmd+J` | ToggleHistory | Toggle history panel |
| `Cmd+H` | ToggleAgent | Toggle agent panel |
| `Cmd+Shift+F` | FormatQuery | Format SQL query |

### Table Tree Navigation (when focused)

| Key | Action | Description |
|-----|--------|-------------|
| `j` / `Down` | NextItem | Move to next item |
| `k` / `Up` | PrevItem | Move to previous item |
| `Enter` / `Right` | ExpandOrSelect | Expand folder or select table |
| `Left` / `h` | Collapse | Collapse folder |
| `gg` | GoToFirst | Jump to first item |
| `G` | GoToLast | Jump to last item |

### Results Panel Navigation (when focused)

| Key | Action | Description |
|-----|--------|-------------|
| `Left/Right/Up/Down` | NavigateCell | Move between cells |
| `Cmd+C` | CopyCell | Copy cell value |
| `Cmd+Shift+C` | CopyRow | Copy entire row |
| `Cmd+Shift+E` | ExportResults | Export results |

### Storage Browser Navigation (when focused)

| Key | Action | Description |
|-----|--------|-------------|
| `j` / `Down` | NextFile | Move to next file/folder |
| `k` / `Up` | PrevFile | Move to previous file/folder |
| `Enter` / `Right` | OpenOrNavigate | Open file or enter folder |
| `Backspace` / `Left` | NavigateUp | Go to parent folder |
| `Cmd+C` | CopyPath | Copy file path |
| `r` | Refresh | Refresh current view |
| `n` | NewFile | Create new file (if supported) |

### SQL Editor (when focused)

| Key | Action | Description |
|-----|--------|-------------|
| `Cmd+Enter` | ExecuteQuery | Execute current query |
| `Cmd+Shift+F` | FormatQuery | Format SQL |
| `Cmd+/` | ToggleComment | Toggle line comment |
| `Tab` | AcceptCompletion | Accept autocomplete suggestion |
| `Escape` | DismissCompletion | Dismiss autocomplete menu |

## Implementation Phases

### Phase 1: Foundation (Current)
- [x] Define action types in `actions.rs`
- [x] Create keybinding registration in `bindings.rs`
- [x] Add global shortcuts (mode switching, quit, help)
- [x] Integrate into `main.rs`

### Phase 2: Panel Navigation
- [ ] Add focus tracking to Workspace
- [ ] Implement panel focus shortcuts (Cmd+E/T/R)
- [ ] Add visual focus indicators

### Phase 3: List Navigation
- [ ] Add j/k navigation to connection lists
- [ ] Add keyboard support to table tree
- [ ] Add keyboard support to storage browser

### Phase 4: Results & Editor
- [ ] Add cell navigation to results panel
- [ ] Add copy shortcuts
- [ ] Enhance editor keyboard support

### Phase 5: Polish
- [ ] Implement help overlay (`?`)
- [ ] Add customizable keymaps (optional)
- [ ] Accessibility improvements

## Focus Management

The workspace tracks which panel currently has focus using `FocusHandle`:

```rust
pub enum FocusedPanel {
    Editor,
    Tables,
    Results,
    Agent,
    History,
    ConnectionList,
    StorageBrowser,
}
```

Focus is managed by:
1. Each panel maintains its own `FocusHandle`
2. Workspace tracks the currently focused panel
3. Shortcuts are context-aware based on focused panel
4. Visual indicators show focused state (border highlight)

## GPUI Integration

Using GPUI's keyboard system:

```rust
// Action definition
actions!(pgui, [ExecuteQuery, ToggleSidebar, ...]);

// Keybinding registration
cx.bind_keys([
    KeyBinding::new("cmd-enter", ExecuteQuery, Some("Editor")),
    KeyBinding::new("cmd-b", ToggleSidebar, None),
]);

// Action handler
cx.on_action(|action: &ExecuteQuery, cx| { ... });
```

## Testing

- Manual testing of all shortcuts in each context
- Verify no conflicts between global and local shortcuts
- Test with different keyboard layouts (if applicable)
- Accessibility testing with screen readers
