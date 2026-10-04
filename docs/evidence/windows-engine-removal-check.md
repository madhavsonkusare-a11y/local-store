# Empty owned-engine removal contract

Current owner CLI controls:

```text
local-store engine stop-supervisor
local-store engine removal-preview
local-store engine remove --consent --confirm local-store-engine-v1
```

Stop-supervisor only requests exit of Local Store's own background pipe lease;
wait for that worker to exit before previewing removal. Preview does not authorize
a later removal. Remove independently repeats every ownership and emptiness check
under cross-process locks and requires the fixed distribution name plus explicit
consent. It deletes the owned engine's cached images and virtual disk.

Removal refuses registered managed Compose apps (including old engine bindings),
retained app setup/data, every container including stopped containers, every Docker
volume, failed/truncated/ambiguous inventories, legacy/corrupt registries, missing
or changed journal/token/selection, foreign or adopted development locations,
missing/unexpected native disk files, links/reparse ancestry, and a running or
starting supervisor. Only the native product data directory is eligible; adoption
of a development engine does not authorize deleting its development location.

The operation holds the engine operation lock, all current offering install-ID
locks, the registry lock and the supervisor spawn/worker locks. This excludes an
install that has not yet committed its registry record. Registry/app data are not
deleted or rewritten. Protocol windows, external app connections, shared host
folders and unrelated engines/distributions survive this transaction.

The command calls the existing strict journal/in-distro ownership verifier, queries
the fixed daemon socket inside the owned distribution, and repeats checks just
before preparing the single fixed `wsl.exe --unregister local-store-engine-v1`
command. It never calls global WSL shutdown, foreign distro termination, Docker
context changes, broad container/volume deletion or recursive host deletion.

A durable `removal-v1.json` record is written before unregistration. Successful
unregistration is followed by a fresh distribution inventory and empty native
host-folder check before exact ownership-file cleanup. If acknowledgement is lost
or the distribution/host files remain, original journal/token/selection survive.
If local cleanup fails after unregistration, original identity bytes are restored
where possible; changed state is never overwritten. The absent distro then
requires manual review instead of a blind destructive retry. Supervisor status/
lock files and the minimal removal record remain; they are not ownership proof.

An unrestricted same-Windows-user process can bypass these locks and modify local
files/daemon directly; this is not a process sandbox. A powered-off disk or failed
filesystem cannot guarantee recovery writes. Refusals preserve evidence and need
owner review rather than a generic force option.

## Verification and acceptance limits

On 4 October 2026, all seven focused removal tests passed, followed by the
explicitly opted-in real fixture transaction. The accepted receipt is
[`windows-engine-removal-2026-10-04.json`](windows-engine-removal-2026-10-04.json).
The transaction imported the pinned rootfs into one new unique fixture, matched
all 138 locked packages, completed the actual removal code and confirmed that
the fixture was unregistered and its virtual disk and ownership files removed.
It completed in 14.915 seconds with 21 WSL calls. The real distribution inventory
remained exactly `docker-desktop` and `local-store-engine-v1`; all four native
identity file bytes remained unchanged. This is an existing-host isolated proof,
with the acceptance limitations below retained.

The focused `engine_removal` tests use temporary owned filesystem fixtures and
injected process results to prove exact confirmation, malformed/legacy registry
refusal, retained app data, saved managed bindings, external-registry preservation,
nonempty/truncated/ambiguous engine inventory, supervisor lock refusal, foreign
selection/adopted location, Windows reparse ancestry, changed recheck state,
unregister failure/lost acknowledgement and identity restoration after local
cleanup failure. Those tests do not unregister the production engine.

The ignored opt-in `actual_unique_fixture_removal_transaction` test uses the
retained exact 138-package rootfs on the existing Windows/WSL host. Its private
namespace adapter maps the fixed product distro argument to a new unique owned
fixture; only that fixture can receive import/in-distro/unregister commands.
Actual full distro inventories and native journal/token/selection/marker bytes
are compared separately before and after. Each command retains its deadline and
the fixture has a five-minute overall budget. Failed/uncertain fixtures keep their
exact marker/journal/token in the ignored proof directory for manual review.

A passing real fixture receipt proves the actual removal code transaction under
this isolated namespace on the existing host. It does not prove deletion of the
production engine, a fixed-name fresh-PC setup/remove flow, clean-host support,
installer removal or complete native WebView acceptance. Current production and
adopted development engines are never unregister targets for this proof.

Run only this opt-in proof after reviewing its unique-fixture adapter and
reserving the engine proof lane. It imports one additional temporary WSL disk;
the exact fixture ownership marker and journal remain available on any failure.

```powershell
$env:LOCAL_STORE_RUN_ENGINE_REMOVAL_PROOF = '1'
cargo test --locked --release --features tauri/custom-protocol,mcp-sidecar --lib engine_removal::real_test::actual_unique_fixture_removal_transaction -- --ignored --exact --nocapture
```

The receipt records the two actual removal source hashes, full real distribution
inventories and hashes of native identity files. Failed attempts have unique
`windows-engine-removal-failed-2026-10-04-<fixture>.json` names; a successful
`windows-engine-removal-2026-10-04.json` cannot overwrite an earlier receipt.
