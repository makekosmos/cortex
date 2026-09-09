# Huawei Health — integration investigation

Target: sign in to Huawei from Kosmos, archive the available account history in
ARK, and keep importing changes. This directory contains the verified protocol
findings, capture tools, worker, and package manifest. Catalog publication and a
fresh native end-to-end login remain pending; no full-history coverage is
advertised.

## Existing integration boundary

- `../greatfrontend/manifest.json`: browser login captures an explicit cookie
  allowlist; its worker imports through brokered `network.fetch` and ARK calls.
- `../../runtime/src/package_manifest/integration.rs`: `BrowserLogin` accepts
  cookie login and the restricted `huawei_health` code-exchange flow.
- `../../runtime/src/package_manifest/integration/secret_injection.rs`: supports
  headers, Basic, query parameters, cookies and mapped JSON session fields.
- `../../runtime/src/package_service/integrations/secret_store.rs`: existing OS
  keyring storage should own the eventual session.
- `../../runtime/src/package_worker_broker.rs`: validates network origins,
  redirects and request/response sizes. A Huawei worker must retain this boundary.

Huawei's inspected Android path puts its access token in JSON `token`, with
`x-huid` and `x-version` headers. Besides HMS sign-in, the APK has an HmsLite
web-login branch (`10414141`, `hms://redirect_url`) with Health backend
`/commonAbility/userAccessToken/obtain` and `/refresh` exchanges. A desktop browser login code was successfully exchanged for access and refresh
tokens; an immediate refresh also succeeded. The home-country endpoint selected
FR, and the EU motion-path endpoint returned 439 distinct workout records over
27 nonempty pages plus an empty terminal page. Raw pages are archived outside
Git with Windows DPAPI. Other health categories and ARK integration remain
unfinished. Copying
GreatFrontend's login URLs and cookie contract would not implement this flow.
Do not add invented OAuth endpoints/cookie names or hand credentials to a worker
as plaintext settings.

The broker's `json` injection maps fields from the opaque stored session. The
verified Huawei data request needs this policy:

```json
{
  "kind": "json",
  "origins": [
    "https://healthdata.dbankcloud.cn/",
    "https://sportdata-dra.things.dbankcloud.com/",
    "https://sportdata-dre.things.dbankcloud.com/",
    "https://sportdata-drru.things.dbankcloud.ru/"
  ],
  "body_fields": {"token": "accessToken"},
  "header_fields": {"x-huid": "uid"},
  "headers": {"x-version": "and_health_16.1.6.320"}
}
```

Workers send the other request fields and an opaque secret handle. The broker
adds the selected session fields, rejects a worker-supplied `token`, and enforces
the secret origin on every redirect. The refresh token stays out of data requests.

Native Manager login now recognizes `code_exchange: "huawei_health"`, with
`start_url: "https://oauth-login.cloud.huawei.com/oauth2/v3/authorize"` and
`completion_url: "hms://redirect_url"`. Runtime generates an expiring state and
nonce. Manager intercepts the callback before OS protocol handling and calls
`integrations.login_complete`; Runtime exchanges the code and publishes the
session through the existing keyring-backed integration setting. Cancellation,
new login attempts and manual session replacement invalidate stale publication.
The current exchange backend is the verified RU endpoint. Rust validation tests,
Manager callback tests and typecheck pass; a fresh native end-to-end login is
still pending, and this directory still lacks an installable worker manifest.

## Capture the missing live evidence

### Verified Windows HTTP prototype

The Python tools now include direct cloud reads, DPAPI page storage, token refresh
and account-bound incremental checkpoints. They use only the Python standard
library and require Windows. Keep archives and session files outside the
repository.

With an existing DPAPI-encrypted session from the login investigation:

```powershell
rtk proxy python tools/archive_account.py C:/private/session.dpapi "C:/private/Huawei Health.apk" C:/private/account-first
rtk proxy python tools/archive_account.py C:/private/session.dpapi "C:/private/Huawei Health.apk" C:/private/account-next --since C:/private/account-first
```

The account command reads verified legacy, point, sequence and statistics streams,
plus selected personal report fields. `--since` accepts an incomplete prior account
batch and searches its parent chain for each stream's last checkpoint. Keep all
parent directories: subsequent batches contain changes, not another full copy.
The command has passed two live incremental runs across 35 nonzero streams: the
first saved 760 additional records; the second saved zero records. Both archived
the requested personal report. This does not establish coverage of every APK API.

Individual stream/category commands remain available:

```powershell
rtk proxy python tools/archive_paths.py C:/private/session.dpapi C:/private/workouts-first
rtk proxy python tools/archive_paths.py C:/private/session.dpapi C:/private/workouts-next --since C:/private/workouts-first
rtk proxy python tools/archive_health.py C:/private/session.dpapi "C:/private/Huawei Health.apk" C:/private/health-first
rtk proxy python tools/archive_health.py C:/private/session.dpapi "C:/private/Huawei Health.apk" C:/private/sequences-first --category sequence
rtk proxy python tools/archive_health.py C:/private/session.dpapi "C:/private/Huawei Health.apk" C:/private/statistics-first --category statistics
```

Each destination must be new. The health command covers category 0 from the APK's
dict_config.txt by default. Sequence and statistics modes query their separate
endpoints; none of these modes covers all account categories. Archives keep original response JSON
encrypted; `complete.json` describes only that stream/batch. A failed batch is
incomplete. `--since` resumes a saved checkpoint and matches account/stream;
checkpoints are updated after each durable, validated page, including in batches
that later fail. The checkpoint preserves the advertised target version so a
resumed incomplete stream cannot accept an early empty response. Only a successful
terminal response produces `complete.json`. Read requests retry Huawei error 1102
at 5, 15 and 30 seconds; persistent errors still leave the batch incomplete.

`receive_hms.py --install` registers a temporary current-user callback handler and
prints a login URL. After browser login, the handler validates the callback,
exchanges its code and writes `tools/login-probe/session.dpapi`; status is recorded
in `tools/login-probe/status.txt`. `--remove` removes only its own registration.
This combined handler flow has an offline test; the code exchange and refresh were
verified separately against Huawei. A fresh end-to-end browser run is still pending.
The scripts are not connected
to the Kosmos login UI or ARK. The broker now supports opt-in chunked network
responses up to 32 MiB; ordinary inline replies retain their existing limits.
JSON token/UID injection, native code exchange and refresh of the keyring-backed
session are implemented. Before Huawei requests, Runtime reloads the current
session and rotates tokens when expiry is within one minute. A keyring journal
recovers interrupted publication; account identity must match the worker handle.
Disconnect removes both the session and journal. Rotation/recovery have local
HTTP test coverage; the native worker flow still needs live acceptance.

For a large page, the worker calls `network.fetch` with `response_mode: "chunks"`
and the usual `url`, `body` and `secret_handle`. It receives `response_handle`,
`size` and `chunk_size`. Subsequent `network.fetch` calls use the same permitted
`url`, `response_handle`, byte `offset` and `length` (at most 262144) and return
base64 `bytes`. A final call with `response_handle` and `close: true` releases the
buffer. Only one response per worker generation may remain open. Stop/restart
cleanup releases its buffers, and other generations cannot read them.
Four buffers and four large downloads may coexist. A synthetic 25 MiB HTTP page
passed through actual worker dispatch and was reconstructed byte-for-byte;
this is not yet a live Huawei worker run.

Offline Windows checks:

```powershell
rtk proxy python tools/receive_hms.py --self-test
rtk proxy python tools/check_login.py
rtk proxy python tools/archive_paths.py --self-test
rtk proxy python tools/check_session.py
rtk proxy python tools/check_checkpoint.py
rtk proxy python tools/check_busy.py
rtk proxy python tools/check_health_archive.py
rtk proxy python tools/check_account.py
```

### Optional Android capture

Requires your own authenticated Huawei Health **16.1.6.320** in an Android
environment where Frida can attach. Ordinary USB debugging alone is not enough.
Install compatible Frida tooling separately; these files do not modify a device.

Run from this directory, replacing `1234` with the actual sync-process PID:

```powershell
rtk proxy frida-ps -Ua
rtk proxy frida -U -p 1234 -l tools/capture.js -o capture.log
```

Wait for `HUAWEI_ARCHIVE_READY`, then perform the app's normal sync and inspect
history. After stopping capture:

```powershell
rtk proxy python tools/extract_capture.py capture.log archive.jsonl
```

The output retains each decoded response as `response_json`, without rounding
64-bit versions. Request headers and credentials are excluded. This is a partial
observation, not a complete account backup: cached data, uploads, other network
methods and unknown fields dropped by the app's response model may be absent.
Use a separate capture per account. Health data is plaintext in these local files;
keep the ignored captures outside version control.

Failed extraction leaves only a `.partial` file. Successful publication requires
a filesystem supporting hard links (for example NTFS). Existing archives are
never overwritten; use a new destination for another capture.

## Acceptance before enabling the integration

1. Verify live requests and responses for `getSyncVersions`, a health page, a
   workout page and a motion-path page. Resolve serialized field names/type codes.
2. Prove the account-login-to-health-token exchange and refresh in the intended
   desktop environment. Android's cached token alone does not prove desktop login.
3. Implement only the proven credential transport in the existing broker, keeping
   the token in the keyring and origin-scoped secret handle.
4. Validate the worker/manifest, per-account/per-stream cursors and raw archival pages.
   Persist pages before advancing cursors, keep deletion events, and fail visibly
   on API errors or stalled cursors.
5. Compare the initial archive with Huawei Health history; add a new record and
   verify the next run, duplicate handling, restart and expired-session behavior.

## Checks

```powershell
rtk proxy node tools/check_capture.cjs
rtk proxy python tools/check_extract.py
```

These are offline checks, not Android/Frida/server acceptance. See
[REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md) for APK evidence and limitations.
