# Menu and Settings Reorganization

**Date:** 2026-08-09  
**Status:** Implemented (2026-08-09) — File/View/Settings menus per Option 4; deviations: gear icon and footer theme toggle retained as quick-access paths

## Problem

Settings and configuration are scattered across four locations with no clear ownership:

| Location | Contents |
|----------|----------|
| File menu | Add MIB Directory, Manage MIBs (2 items) |
| Gear icon (TargetBar) | SNMP version, community string, v3 auth/priv settings, Test Connection |
| Results header | Save, Clear, MIB Names/Raw OIDs, Wrap, Filter |
| Footer | Theme toggle, System Log toggle |

A 2-item File menu doesn't justify a top-level menu. Meanwhile, the gear icon is hard to discover, and footer toggles are app-scoped settings hiding in a status bar.

## Options Considered

### Option 1: Buttons next to MIB browser title, remove File menu

Replace the File menu dropdown with inline buttons beside the MIB panel title, matching the Results header pattern.

| Pros | Cons |
|------|------|
| Consistent with existing inline-controls pattern | 2 buttons waste horizontal space in the most constrained area (MIB panel) |
| Controls are visible and scannable | Loses keyboard shortcut potential (`Ctrl+O`) |
| Reduces "where do I find X?" cognitive load | Premature — only pays off at 5+ items, not 2 |

### Option 2: Everything in the File menu

Consolidate all ~16 settings into a single expanded File menu.

| Pros | Cons |
|------|------|
| Single source of truth for settings | Wall of text — connection fields, display toggles, theme, log level don't belong together |
| Standard desktop pattern | Conditional UI (v3 fields) doesn't translate to a flat menu |
| | "Test Connection" button in a dropdown is awkward |
| | Users expect display toggles near the content they affect |

### Option 3: Both — duplicate access paths

Provide settings through both inline controls and the File menu.

| Pros | Cons |
|------|------|
| Flexibility | Two paths to the same setting creates confusion |
| | Double implementation and maintenance surface |
| | UX anti-pattern unless each path serves a distinctly different context |

### Option 4 (Recommended): File + Settings menus, inline stays inline

Introduce a **Settings** menu alongside the existing **File** menu. Keep content-scoped controls inline where they are today.

```
File                          Settings
├── Add MIB Directory...      ├── Connection...
└── Manage MIBs...            ├── Theme
                              └── System Log Level
```

Content-scoped toggles (MIB Names/Raw OIDs, Wrap, Filter) remain in the Results header — they affect one view, not the whole app.

| Pros | Cons |
|------|------|
| File menu stays focused on data operations (MIB files) | Requires one new menu component |
| Settings menu collects app configuration currently scattered between gear icon and footer | Need to decide if Connection settings open as submenu or modal |
| Inline toggles stay where users expect them | |
| Gear icon disappears — replaced by discoverable top-level menu | |
| Clear home for future features (export defaults, keybindings, etc.) | |

## Recommendation

**Option 4.** Migration path:

1. Move connection settings from gear icon → Settings > Connection (opens existing `ConnectionModal`)
2. Move theme toggle from footer → Settings > Theme
3. Move system log level filter → Settings > System Log Level
4. Keep Results header toggles inline — they are content-scoped, not app-scoped
5. File menu stays untouched

Net result: 2 menus with clear ownership, no duplication, gear icon removed.

## Open Questions

- Should Connection settings open as a modal (current behavior) or render inline in the Settings dropdown?
- Should the System Log toggle stay in the footer for quick access, with level control moving to Settings?
- Future: does Settings need a submenu structure as it grows (e.g., Display, Keyboard Shortcuts)?
