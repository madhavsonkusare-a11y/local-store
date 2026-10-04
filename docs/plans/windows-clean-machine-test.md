# Clean Windows test on Madhav's laptop

> Reviewed for shelving October 4, 2026. Development is paused. This is an architecture/method reference with dated checkpoints; current scope and acceptance are in [V1_TASKS.md](../V1_TASKS.md). Local build artifacts and the development engine were removed.

October 2, 2026. This is an R04 test plan, not a completed installation test.
The owner asked whether a Windows VM could be used on the laptop and how much
storage it needs. No VM, Windows edition upgrade or host feature change has
been performed.

## Measured host and proposed budget

The host reports Windows 11 Home Single Language, 15.8 GiB RAM, eight logical
processors and an active hypervisor. D: has 229.1 GiB free; C: has 74.5 GiB free.
Keep the test files on D:, under a dedicated `D:\06 Projects\_build\windows-vm`
directory, outside the source checkout. Quote this path in scripts.

Plan for a dynamically growing 120 GiB Windows disk, 8 GiB guest RAM and four
virtual processors. Reserve roughly 100–120 GiB of free host storage for the
Windows installation, ISO, WSL disk, app images and one baseline checkpoint.
This is a planning budget, not a measured Windows installation footprint.
The disk grows with use; a 120 GiB virtual capacity does not allocate all of it
at creation. Large media libraries and additional checkpoints need more space.
Run app checks sequentially, and transfer a built development artifact rather
than compiling the complete Rust/browser toolchain inside the guest.

Windows 11 itself requires at least 64 GB storage and 4 GB RAM. Those minimums
do not include Local Store's apps or testing overhead. See [Windows 11
requirements](https://www.microsoft.com/en-us/windows/windows-11-specifications).

## Current host limitation

The supported Hyper-V host role cannot be installed on Windows 11 Home. A
Windows 11 Pro/Enterprise host with supported hardware is the straightforward
Microsoft-supported route for this particular nested-virtualization test.
See [Hyper-V installation](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/get-started/install-hyper-v?pivots=windows)
and [nested virtualization](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/enable-nested-virtualization).

An ordinary Windows guest is insufficient: Local Store needs its WSL2 Linux
engine **inside** that guest. VMware documents that enabling guest Hyper-V/WSL2
nested virtualization conflicts with an enabled Hyper-V host. The current host
already uses its hypervisor for WSL2. Do not disable host virtualization/security,
stop global WSL, apply unsupported Home-edition feature workarounds, buy an
upgrade or start a paid cloud machine as an incidental test setup. See
[VMware's nested-hypervisor constraints](https://knowledge.broadcom.com/external/article?articleNumber=313547).

This limitation is about hosting the test VM. Local Store already runs its
owned WSL2 engine on this Home host; it does not establish that users need Pro.
The owner can choose a supported host upgrade, another suitable Windows PC,
or a separately approved remote test machine. Until then R04 stays unverified.

## Execution contract once a supported host is available

1. Create an isolated Generation 2 Windows 11 VM with Secure Boot and virtual
   TPM using an official ISO and applicable Windows license. Expose supported
   virtualization extensions while it is powered off. Keep a baseline with no
   Local Store, WSL distro, Docker Desktop or development prerequisites.
2. Copy the exact reviewed unsigned **private development** artifact and its
   digest into the guest. This is not a public installer release. Capture the
   guest Windows/WSL versions, free storage and pre-existing distro inventory.
3. First exercise the missing/restricted prerequisite path with a standard
   user: explain administrator/restart requirements; refusal must not silently
   enable features, import a distro or select an ambient daemon.
4. With explicit setup consent, install the supported WSL prerequisites,
   restart when required, and run the actual native launcher setup transaction.
   Measure import/readiness time and disk usage. Verify the fixed product distro,
   ownership token, exact payload and locked package inventory.
5. Run the native path through install, useful app task, exact-scoped agent
   access, launcher close/reopen, machine restart, recovery and keep-data
   uninstall/reinstall. Reuse the existing ten-app qualification probes;
   do not count an image pull or screenshot as a useful app task.
6. Verify protocol/shortcut launch and native app windows. Test sleep/wake,
   exact worker recovery and coexistence with a deliberately separate distro.
   Remove only the token-verified Local Store engine after protecting app data;
   assert the unrelated distro and host state are unchanged.
7. Retain a receipt bound to artifact/payload/probe hashes and actual outcomes.
   Missing native automation remains explicit; browser fixtures cannot stand in
   for the Windows WebView. Restore only this guest's baseline for a rerun.

## Existing-host proof is separate

The [fresh-payload receipt](../evidence/windows-fresh-payload-2026-10-02.json)
records a new temporary WSL2 import on the existing host: 138 exact packages,
Docker 29.8.0, Compose 5.5.1, active systemd Docker, 28.84 seconds to ready and
a 683,671,552-byte VHD file after boot. Its exact token-verified fixture was
unregistered, and the selected product engine state remained unchanged.
This proves the retained payload boots; it does not prove clean Windows setup,
the native fixed-name transaction, failed prerequisites or a minimum disk size.
