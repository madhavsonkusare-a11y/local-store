# Support and recovery

Local Store is maintained as a preview project by one person. There is no
support service-level agreement, guaranteed response time or bounty. The public
[source V1 release](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0)
contains no Windows installer. Unsigned personal builds are development previews.

Windows x64 is the active product target. Current evidence comes from an existing
Windows host with WSL prerequisites already available. It does not establish a
supported Windows/WSL version floor, fresh-PC bootstrap, restricted prerequisites,
complete current installer removal, or native WebView acceptance of every flow.
Docker Desktop is not a prerequisite for the current owned-engine proofs.

## Before changing or removing data

Use the launcher status and recovery inspection before retrying an interrupted
operation. Setup/repair/adoption is explicit and refuses uncertain ownership.
Do not manually delete the registry, engine journals, WSL distribution or shared
folders to make a refusal disappear. Stop and report the refusal if ownership
cannot be established. A desktop-app uninstall does not itself prove removal of
the shared engine or all app data. The owner CLI has a separate guarded
[empty owned-engine removal path](evidence/windows-engine-removal-check.md); it
refuses apps, retained data, containers, volumes and uncertain ownership.
Adopted development-engine locations are preserved. Its isolated fixture proof
is separate from clean-PC/product installer removal acceptance.

Keep-data uninstall retains generated credentials and app data for reinstall.
Delete-data removal is permanent for that owned app folder. Removing a linked
connection removes its registry record, not remote data or WebView cookies/cache.
Owned containers, named volumes and the shared engine have separate lifecycles.

Protected snapshots use Windows current-user DPAPI and restore only to an empty,
unoccupied, verified owned target. Actual Memos and Gitea proofs cover consistent
local content and exact credentials, including Gitea named volumes. Shared host
folders are refused. This is not portable account-loss disaster recovery, automatic
app-version rollback, or undo for external email/service effects. Keep a separate
appropriate recovery plan for shared folders and external services.

## App and agent limits

53 offerings are available; the selected ten have current managed-engine task and
bounded content-access proof. Linkding has separate candidate app-task proof,
with content access unverified. Consult the [agent action/setup matrix](evidence/launch-agent-coverage-2026-10-02.md)
for exact read/write limits. Larger workloads and later app versions require new
qualification. Set strong app credentials; Kanboard's initial administrator
password must be changed immediately, and its plugin installer is optional code
execution from the internet.

Client enrollment, app connection and an exact expiring client/app/action grant
are separate steps. Each supported write or install/uninstall needs its own exact
owner approval. Revoke the client and disconnect the app if access should stop.
App text can contain malicious instructions: the broker treats it as data, while
the external agent's judgment remains outside this app's proof. An unrestricted
process under the same Windows account can bypass broker permissions through
files or the engine; these permissions do not sandbox that process.

## Reporting a problem safely

For an ordinary bug, use the repository's [issue page](https://github.com/madhavsonkusare-a11y/local-store/issues).
Include the product/source commit, Windows edition/version, WSL version if relevant,
app ID, operation and stage, expected/observed result, and smallest reproducible
steps. Say whether this is source-run, a local preview or a downloaded artifact.
For an artifact, include its filename and SHA-256; a hash establishes identity,
not publisher trust or support certification.

Review every attachment before posting. App logs are verbatim and may contain
passwords, tokens, personal content, private URLs or paths. Diagnostic redaction
does not sanitize arbitrary app logs, screenshots or exported client configuration.
Do not attach Compose files, `local-store-secrets.json`, connection credentials,
client configuration, browser profiles or private backup/data archives. Describe
a sensitive failure with inert examples; share private material only through a
separately agreed secure route when necessary.

For a vulnerability, use private GitHub security advisory reporting on
[the repository](https://github.com/madhavsonkusare-a11y/local-store).
If private reporting is unavailable, do not publish exploit details or credentials
in a public issue; request a private reporting route without sensitive details.
See [security and privacy](security-and-privacy.md) for enforced boundaries and
[PUBLISH.md](../PUBLISH.md) for release verification limits.
