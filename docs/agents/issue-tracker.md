# Issue Tracker

Issues live as Markdown files under `~/git/scout-tickets/` — outside the repo to keep the workspace clean.

## Format

Each issue is a single `.md` file named `<number>-<slug>.md`. Features get their own subdirectory:

```
~/git/scout-tickets/
├── mib-resolution/
│   ├── 001-fallback-parser.md
│   └── 002-oid-autocomplete.md
└── snmp-engine/
    └── 003-table-detection.md
```

## Workflow

Skills like `to-tickets`, `triage`, `to-spec`, and `qa` read from and write to this directory. Numbering is sequential within each feature folder. No external CLI (`gh`, `glab`) is needed — all operations are file-based.
