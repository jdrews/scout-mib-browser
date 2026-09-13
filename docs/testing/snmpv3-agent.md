# SNMPv3 Test Agent (snmpsim)

A [snmpsim](https://pypi.org/project/snmpsim/) command responder pre-configured with one SNMPv3 USM user per protocol combination, so you can exercise every supported auth/priv protocol against a live agent. The Rust integration tests in `crates/scout-snmp/tests/snmpv3_integration.rs` run against this same agent.

## Start it

```bash
npm run test:e2e:agent:v3                 # starts on port 11700
# or directly:
python3 scripts/snmpsim-v3-test.py --port 11700
```

The agent serves the default SNMPv3 context (empty context name) from the bundled `linux-full-walk.snmprec` recording. Stop it with `Ctrl+C`, or if you backgrounded it: `pkill -f "snmpsim-command-respon[d]er"`.

> Requires `snmpsim` (`pip install snmpsim`) and its `linux-full-walk.snmprec` recording in the package data dir.

## Users & credentials

The agent does **not** accept arbitrary credentials — it only answers requests that match one of the users below at the right security level. Each user is bound to a specific auth + priv protocol pair, so pick the row for the combination you want to test.

| User | Level | Auth (proto / passphrase) | Priv (proto / passphrase) |
|------|-------|---------------------------|---------------------------|
| `v3_noauth` | noAuthNoPriv | – | – |
| `v3_md5` | authNoPriv | MD5 / `auctoritas-md5` | – |
| `v3_sha` | authNoPriv | SHA-1 / `auctoritas-sha` | – |
| `v3_sha224` | authNoPriv | SHA-224 / `auctoritas-sha224` | – |
| `v3_sha256` | authNoPriv | SHA-256 / `auctoritas-sha256` | – |
| `v3_sha384` | authNoPriv | SHA-384 / `auctoritas-sha384` | – |
| `v3_sha512` | authNoPriv | SHA-512 / `auctoritas-sha512` | – |
| `v3_des_md5` | authPriv | MD5 / `authpass-md5` | DES / `privpass-des` |
| `v3_aes128_sha` | authPriv | SHA-1 / `authpass-sha` | AES-128 / `privpass-aes128` |
| `v3_aes192_sha256` | authPriv | SHA-256 / `authpass-sha256` | AES-192 / `privpass-aes192` |
| `v3_aes256_sha512` | authPriv | SHA-512 / `authpass-sha512` | AES-256 / `privpass-aes256` |
| `v3_aes192_md5` | authPriv | MD5 / `authpass-md5ext` | AES-192 / `privpass-aes192ext` (Reeder key-extension) |

## Try it

**In the Scout app (primary):** enter any row's username, auth protocol + passphrase, and priv protocol + passphrase in the connection panel, then run Test Connection. The Rust SNMP engine supports every protocol above — this is the definitive way to verify a combination.

**With net-snmp `snmpget` (CLI spot-checks):** requires a net-snmp build compiled with full SNMPv3 auth + privacy support.

```bash
# noAuthNoPriv
snmpget -v 3 -u v3_noauth localhost:11700 .1.3.6.1.2.1.1.1.0

# authNoPriv, SHA-256
snmpget -v 3 -u v3_sha256 -a SHA256 -A auctoritas-sha256 localhost:11700 .1.3.6.1.2.1.1.1.0

# authPriv, AES-128 + SHA   (net-snmp tokens: AES = AES-128, also AES192 / AES256)
snmpget -v 3 -u v3_aes128_sha -a SHA -A authpass-sha -x AES -X privpass-aes128 localhost:11700 .1.3.6.1.2.1.1.1.0
```

> If `snmpget` prints **`Unsupported security level`** (or rejects `-x DES`), your net-snmp build is missing that v3 feature — it's a client limitation, not an agent or credential problem. Verify the combination in the Scout app instead.

## Notes

- **Keep in sync:** the user matrix lives in `scripts/snmpsim-v3-test.py` (`V3_USERS`) and must match `crates/scout-snmp/tests/snmpv3_integration.rs`. Add a new combination to both if you extend coverage.
