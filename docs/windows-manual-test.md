# Personal Windows installer test (archived)

> Shelved October 4, 2026. The setup file, installed app and managed engine mentioned below were removed at the owner's request. These are historical instructions; rebuild and update the artifact details before using this guide.

Prepared October 4, 2026. This is a private development preview for Madhav's
existing Windows laptop. The public V1 release remains source-only.

Setup file: `D:\06 Projects\dockwrap\target\release\bundle\nsis\Local Store_0.5.0-1_x64-setup.exe`

Size: 18,307,828 bytes (about 17.5 MiB).
SHA-256: `5aa36857e5480d53a55826b8a98165341a2a1048da2e318a187de8318d227972`.
The finished archive's resources, notices and entrypoints were inspected. The
installer has not yet been executed or installed by this session.

## Install and first launch

1. Close any running Local Store launcher, then run the setup file under your
   usual Windows account. Record any warning or installation error. This personal
   build is unsigned, so Windows may display an unknown-publisher warning.
2. Open Local Store from the Start menu. Check that its dark interface loads,
   the catalog has icons, and Overview, Discover, My Apps, Activity, Settings
   and Agent connections can be opened using both the mouse and keyboard.
3. Open Settings and check the managed engine. Your existing Local Store engine
   should be available. If offered, choose **Use existing Local Store engine**
   or **Use Local Store engine for installs** and review the consent first.
   Docker Desktop is not required.
4. If the engine is unavailable, capture the exact message and stop that step.
   Do not recreate/delete its state to force the test through. This installer
   contains the launcher and agent connector, **not a fresh engine payload**;
   it tests reuse of the already prepared engine on this laptop.

## Ten launch apps

Install one app at a time from its review screen. Open it using Local Store,
complete its first-use setup, perform the task below, then stop/start it and
check that your sample content remains. Stop heavier apps before testing the
next one. Only these ten are the current launch acceptance cohort; the larger
catalog does not have the same completed proof.

| App | Useful task | Install / open / task / stop-start result |
| --- | --- | --- |
| Memos | Create and reread a private memo | |
| Flatnotes | Sign in, create a note and find it again | |
| Uptime Kuma | Add a monitor and see its reported status | |
| PrivateBin | Create a non-burning sample paste and open its link | |
| Kanboard | Set your own strong admin password, create and move a task | |
| n8n | Create a small workflow and see its execution result | |
| Gitea | Create a private repository and commit a sample file | |
| WordPress | Publish a sample post and view it | |
| Jellyfin | Add your own small media sample and play it | |
| Immich | Upload your own sample photo, search for it and download it | |

PrivateBin pastes expire by design. Choose a lifetime that covers your test;
an intentionally expired paste is not a persistence failure.

## Launcher and recovery checks

- With a sample app running, close and reopen Local Store. Check that the app
  remains usable and its saved entry/status is correct.
- Open a native app window, inspect logs, and create/open an app shortcut.
- For one app containing only disposable sample data, uninstall with **keep
  data**, reinstall and confirm its sample content remains. Test permanent
  deletion separately only on disposable test data; record the exact-name prompt.
- If convenient, put the laptop to sleep yourself, wake it and recheck the
  running app and launcher. Record whether repair was needed.
- In Agent connections, enroll a test client, grant only one app/action, perform
  one supported read and revoke the client. Record the client used and whether
  access stops. Installing an app alone does not grant agents content access.
  The [agent coverage table](evidence/launch-agent-coverage-2026-10-02.md) gives
  each app's required connection method; no need to expose credentials in feedback.
- After completing other checks, optionally uninstall the desktop application
  through Windows Settings, reinstall this same setup and check saved apps/data.
  Do not separately remove the engine or app folders for this test.

## Feedback to send back

Reply with Windows edition/version, whether setup and Start-menu launch worked,
the completed app rows, and which launcher/recovery/agent checks you performed.
For a failure, include the app, step, exact error and a screenshot with private
content/credentials removed. State which checks were skipped rather than treating
them as passes. No need to send passwords, tokens or private app data.

These results count as manual installation/native-use evidence on this laptop.
They do not prove fresh-PC engine bootstrap, missing Windows prerequisites or
clean-machine behavior. Task statuses will be updated from the actual results;
providing the installer or starting this test does not itself complete a gate.
