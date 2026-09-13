# Scout MIB Browser

A fast, free, open-source SNMP MIB browser for Linux, Windows, and macOS. Point it at a network device, browse its MIBs, run queries, and watch live results stream in. Built with Svelte + Rust via Tauri.

<!-- Screenshot -->

## What it does

Routers, switches, servers and many other devices expose their state through SNMP with thousands of OIDs defined in MIB files. Scout makes that data easy to explore:

- **Browse any MIB**: browse/search the MIB tree and inspect OIDs; works out of the box with bundled IETF MIBs or add your own MIBs.
- **Query real devices**: SNMP v1, v2c, and v3 with Get, GetNext, Walk, BulkWalk, Get Table, and Set. Results stream in live as they arrive, so big walks stay responsive.
- **Tables as grids**: fetch a whole MIB table in one pass and view it as sortable rows and columns, not a flat OID dump.
- **Forgiving of flaky devices**: Scout never fails silently. Timeouts are retried, partial results are shown with warnings, and malformed responses are decoded as far as possible so you always see what the device actually sent.
- **See the raw bytes**: byte values get hex + ASCII dumps, with MAC addresses and IP addresses recognized automatically.
- **Export anything**: save results as TSV, JSON, or CSV for spreadsheets, scripts, or tickets.

## Development

To build, run, or test the project from source, see [DEVELOPMENT.md](DEVELOPMENT.md).

## License

MIT. See [LICENSE](LICENSE).
