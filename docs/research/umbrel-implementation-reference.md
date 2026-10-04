# Umbrel implementation reference

> Historical research, indexed October 4, 2026. The project is paused. This study is not current release scope; use [V1_TASKS.md](../V1_TASKS.md). Recheck upstream facts before adopting its proposals.

Expanded September 9: see [the backend coverage plan](../backend-app-coverage-plan.md)
for the repository/dependency/gateway/credential/hook architecture comparison,
newly pinned app-store sample review, and execution priorities.

Reviewed September 8, 2026 at Umbrel revision
`bfa79ed24031b0065dd2f810411d58b82af1b95e`. Architectural reference only;
no Umbrel implementation code or assets were copied into Local Store.

## What the code does and what to carry forward

- [Store API](https://github.com/getumbrel/umbrel/blob/bfa79ed24031b0065dd2f810411d58b82af1b95e/packages/umbreld/source/modules/apps/app-store.ts): explicitly selects presentation
  and install-planning metadata rather than sending raw repository manifests to
  clients. It also reports manifest-version compatibility. Local Store should
  keep its catalog schema boundary when adding imported plan previews; never
  expose setup secrets or arbitrary source fields in discovery responses.
- [Install review](https://github.com/getumbrel/umbrel/blob/bfa79ed24031b0065dd2f810411d58b82af1b95e/packages/ui/src/modules/app-store/install-review-dialog.tsx): conditionally presents required
  apps, folder choices and GPU information. In Local Store, use the same general
  principle of showing applicable choices only. A one-click app with no inputs
  should not acquire a mandatory configuration wizard. Do not adopt host-folder
  or GPU capabilities merely because another store offers them.
- [App lifecycle](https://github.com/getumbrel/umbrel/blob/bfa79ed24031b0065dd2f810411d58b82af1b95e/packages/umbreld/source/modules/apps/app.ts): validates and persists installation selections
  before generating Compose and invoking lifecycle scripts. Storage-operation
  coordination prevents conflicting access during changes. Local Store already
  has typed template resolution and transaction locks; extend those, preserving
  validation on the backend and the lock through registry commit.
- The same lifecycle code has explicit transition handling and storage recovery.
  Its uninstall path can leave inaccessible storage behind rather than deleting
  through an unverified path. Local Store's next recovery action should similarly
  revalidate registry state, ownership and retained definition under its lock.

These observations do not establish that Umbrel's hooks or proxy services can
run unchanged on a Windows desktop. The app definitions previously inspected
use platform-injected proxy/auth and hostname settings. See the
[source study](import-sources-study.md) for pinned Memos/n8n examples.

## Source reuse boundary

Umbrel's main code repository has a
[PolyForm Noncommercial license](https://github.com/getumbrel/umbrel/blob/bfa79ed24031b0065dd2f810411d58b82af1b95e/LICENSE.md).
That is distinct from the `umbrel-apps` repository, whose packaging reuse
permission was not established in the earlier review. Neither finding prevents
studying architecture. Bundling implementation or app-store assets requires a
separate reuse decision; this batch uses our existing code and the Compose spec.

## Implementation in this batch

Added typed container health checks to `PlanOverrides` and the CapRover adapter,
following the [Compose specification](https://docs.docker.com/reference/compose-file/services/#healthcheck).
Exec and shell checks, explicit disabling, image-test inheritance, timing fields
and retry counts are retained. Unknown options and unresolved dollar expressions
are refused. This is preservation of container probe configuration, not a new
claim that an app is ready: the launcher's HTTP probe is unchanged, and dependency
conditions such as `service_healthy` still need separate modelling.

Follow-up completed: Runtipi healthCheck and service_healthy dependencies now map
explicitly, and an isolated Adminer/Postgres proof verifies the web service starts
after the delayed database health check passes. Template validation now includes
deployment policy, correcting earlier Runtipi expressibility overcounts. Keep
manifest expressibility separate from verified one-click availability in the UI.
