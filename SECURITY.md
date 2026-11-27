# Security Policy

## Reporting a Vulnerability

Please report security vulnerabilities privately via GitHub
([github.com/jdrews/scout-mib-browser](https://github.com/jdrews/scout-mib-browser)
→ Security → Report a vulnerability). Please do not open a public issue.

## Attack Surface

Scout MIB Browser renders attacker-controlled strings **by design**: MIB
`DESCRIPTION` text comes from user-loaded MIB files, and SNMP value octets
come from arbitrary devices on the network. The webview is therefore treated
as a hostile component:

- **No path-shaped file writes.** The backend exposes no command that takes a
  file path from the frontend. Saving an export opens the native save dialog,
  and the write is scoped to exactly the path the *user* picked there. The
  suggested filename (built from MIB node names) is reduced to a bare name (path
  components, control characters and leading dots are stripped) so it
  can only prefill the dialog, never aim it.
- **Content Security Policy.** The webview runs with
  `default-src 'self'; script-src 'self'` (plus `style-src 'unsafe-inline'`,
  required by Svelte's runtime style injection). Scripts from MIB text, SNMP
  payloads, or injected markup cannot execute.
- **Minimal capabilities.** The shipped capability set is `core:default`
  plus event listening. The WebDriver plugin used by the e2e suite has its
  own capability file that is not referenced from the app config, so it
  never ships.
- **No test code in releases.** The mock SNMP agent (a UDP listener) is
  behind a cargo feature that release builds do not enable.
- **Credentials off by default.** See the next section.

## Credential Storage

Scout MIB Browser stores Target settings in `config.toml` under the user's
config directory (`~/.config/scout/` on Linux, `$XDG_CONFIG_HOME/scout/` when
set, `%APPDATA%\scout\` on Windows). SNMP community strings and SNMPv3
passphrases are stored **plaintext**; SNMP has no at-rest secret store, and
the app deliberately does not pull in an OS keychain for the beta. The
storage posture is therefore built around *not writing secrets at all*
unless the user opts in:

- **Off by default.** `ui.save_credentials` defaults to **false** on fresh
  installs: host, port, version, and protocol choices persist, but
  community strings, v3 usernames, and passphrases never leave memory.
  Typed credentials work for the session and must be re-entered on the
  next launch.
- **Opt-in, honest copy.** The connection dialog states exactly what is
  saved. Turning the toggle off scrubs already-saved credential values
  from the file immediately.
- **Owner-only on unix.** The config file is written `0600` and the
  config directory `0700`. The first save after upgrading from a version
  that wrote `0644` tightens the existing file.
- **Windows ACL posture.** On Windows the config lives under
  `%APPDATA%\scout\`, which inherits the per-user profile ACL: the
  interactive user and `SYSTEM` have access, other local accounts do not.
  Scout relies on that inherited ACL and does not modify it; on a machine
  where `%APPDATA%` ACLs have been loosened, keep the toggle off.
- **Atomic writes.** Saves write a temp file and rename it into place, so
  a crash mid-write cannot leave a truncated config.

**On shared machines (jump boxes, shared build hosts):** keep
`save_credentials` off. v1/v2c community strings travel in plaintext on
the wire and v3 passphrases in the config are recoverable by anyone who
can read the file; the file is owner-only, so the remaining exposure is
your own account.
