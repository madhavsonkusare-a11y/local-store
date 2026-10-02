"""Stage the pinned official browser helper package for development/source builds.

No application token, user browser profile or Docker socket is used. Runtime
image download happens separately through the explicitly selected owned engine.
"""
import base64, hashlib, io, json, pathlib, tarfile, urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = "1.62.1"
INTEGRITY = "wPYSwEBJY9GHraISXqyqtx0na0LpO3XEX7jNDhntbex7tzUS7kLnZsOlFruFJB4Hi/rhDMjXGqHewDZ68nYZVw=="
PROFILE_SHA = "cc3e61cabda6bbc1e53e54d27ba4d55a9d3be829b6dd1a596f4a7b31b1cc7849"
DERIVED_PROFILE_SHA = "8dff2d2308d7af9b30c020f3df33d374405604bc1b300fd45f739f8f2c8f5ead"
PACKAGE_SHA = "6e4e424e7d4651b64e250707f41548e71ef0620e61252b39f1a91d2354e0ece0"

def digest_tree(root):
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*"), key=lambda p: p.relative_to(root).as_posix()):
        if path.is_symlink():
            raise ValueError("runtime symlinks are not accepted")
        if path.is_file():
            digest.update(path.relative_to(root).as_posix().encode() + b"\0")
            digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()

def main():
    for name, expected in [("seccomp_profile.upstream.json", PROFILE_SHA), ("seccomp_profile.json", DERIVED_PROFILE_SHA)]:
        if hashlib.sha256((ROOT / "providers/privatebin" / name).read_bytes()).hexdigest() != expected:
            raise ValueError("reviewed browser sandbox profile integrity mismatch")
    destination = ROOT / "target/release/browser/playwright-core"
    if destination.exists():
        if digest_tree(destination)!=PACKAGE_SHA:
            raise ValueError("existing runtime tree differs from the reviewed package")
        print("Existing browser package tree", digest_tree(destination))
        return
    data = urllib.request.urlopen(f"https://registry.npmjs.org/playwright-core/-/playwright-core-{VERSION}.tgz", timeout=30).read(16*1024*1024+1)
    if len(data)>16*1024*1024 or base64.b64encode(hashlib.sha512(data).digest()).decode()!=INTEGRITY:
        raise ValueError("browser package integrity mismatch")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        total = 0
        for item in archive:
            name = pathlib.PurePosixPath(item.name)
            if name.parts[0]!="package" or any(p in (".","..") for p in name.parts) or name.is_absolute():
                raise ValueError("unsafe package path")
            if item.isdir():
                continue
            if not item.isfile() or item.size>8*1024*1024:
                raise ValueError("unsupported package member")
            relative = pathlib.Path(*name.parts[1:])
            if str(relative) in files:
                raise ValueError("duplicate package member")
            total += item.size
            if total>64*1024*1024 or len(files)>=4096:
                raise ValueError("package exceeds bounded provider payload")
            files[str(relative)] = archive.extractfile(item).read()
    if json.loads(files["package.json"])["version"]!=VERSION:
        raise ValueError("wrong browser package version")
    destination.mkdir(parents=True)
    for relative, data in files.items():
        path=destination/relative; path.parent.mkdir(parents=True,exist_ok=True); path.write_bytes(data)
    if digest_tree(destination)!=PACKAGE_SHA:
        raise ValueError("staged runtime tree differs from the reviewed package")
    print("Staged official browser package", VERSION, len(files), total, digest_tree(destination))

if __name__=="__main__":
    main()
