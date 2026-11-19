"""Package the optimized normal app and verify a macOS extraction before delivery."""
import hashlib
import json
import os
import plistlib
import shutil
import stat
import subprocess
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "entregas" / ("v" + json.loads((ROOT / "package.json").read_text())["version"])
APP = "Stretch & Water.app"
FOLDER = "Stretch and Water"
MARKER = ".stretch-water-project"
EXEC = "Contents/MacOS/stretch-and-water-poc"
FILM_FILES = {f"films/{kind}/{name}" for kind in ("stretch", "water")
              for name in ["durations.json", *(f"{i:03d}.png" for i in range(241))]}
APP_FILES = {EXEC, "Contents/Resources/cling.wav", "Contents/Resources/lotus.icns", "Contents/Resources/reserva.png",
             "Contents/Info.plist", "Contents/_CodeSignature/CodeResources"}
APP_FILES.update(f"Contents/Resources/{name}" for name in FILM_FILES)
FILES = {f"{APP}/{name}" for name in APP_FILES} | {MARKER, "LEEME.txt"}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_folder(folder, version):
    files = set()
    for path in folder.rglob("*"):
        require(not path.is_symlink(), f"Enlace inesperado: {path.name}")
        if path.is_file():
            files.add(path.relative_to(folder).as_posix())
    require(files == FILES, f"Inventario inesperado: {files ^ FILES}")
    app = folder / APP
    binary = app / EXEC
    blob = binary.read_bytes()
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    expected = {"CFBundleShortVersionString": version, "CFBundleVersion": version,
                "CFBundleIdentifier": "com.stretchandwater.poc001", "LSUIElement": True,
                "LSMinimumSystemVersion": "12.0", "CFBundleExecutable": Path(EXEC).name}
    require(all(info.get(k) == v for k, v in expected.items()), "Info.plist inesperado")
    require(os.access(binary, os.X_OK), "El ejecutable perdió su permiso")
    arch = subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip()
    require(arch == "arm64", f"Arquitectura inesperada: {arch}")
    require((folder / MARKER).read_bytes() == (ROOT / MARKER).read_bytes(), "Marcador incorrecto")
    require(sha(app / "Contents/Resources/cling.wav") == sha(ROOT / "src-tauri/assets/cling.wav"), "Audio incorrecto")
    resources = app / "Contents/Resources"
    require(info.get("CFBundleIconFile") == "lotus.icns" and
            sha(resources / "lotus.icns") == sha(ROOT / "src-tauri/icons/lotus.icns"), "Ícono de la app incorrecto")
    require((ROOT / "src-tauri/icons/lotus-tray.rgba").read_bytes() in blob, "Falta la figura de la barra")
    for name in FILM_FILES | {"reserva.png"}:
        require(sha(resources / name) == sha(ROOT / "src-tauri/assets" / name), f"Recurso visual incorrecto: {name}")
    require(b"com.stretchandwater.poc001.login" in blob, "Perfil normal incorrecto")
    require(b"/Users/" not in blob and str(ROOT).encode() not in blob, "Ruta personal en el ejecutable")
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    return {name: sha(folder / name) for name in sorted(FILES)}


def package():
    version = json.loads((ROOT / "package.json").read_text())["version"]
    built = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "src-tauri/target")) / "release/bundle/macos" / APP
    OUT.mkdir(exist_ok=True, parents=True)
    with tempfile.TemporaryDirectory(dir=OUT, prefix=".preparar-") as tmp:
        work = Path(tmp)
        folder = work / FOLDER
        folder.mkdir()
        shutil.copytree(built, folder / APP, symlinks=True)
        shutil.copyfile(ROOT / MARKER, folder / MARKER)
        shutil.copyfile(ROOT / "docs/LEEME-INSTALACION.txt", folder / "LEEME.txt")
        # Finish removing local debug paths if the Rust toolchain's strip failed.
        subprocess.run(["/usr/bin/strip", "-S", "-x", str(folder / APP / EXEC)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none", str(folder / APP)], check=True)
        inventory = verify_folder(folder, version)
        archive = work / f"Stretch-and-Water-{version}-macOS-arm64.zip"
        with zipfile.ZipFile(archive, "w") as out:
            for name in sorted(FILES):
                entry = zipfile.ZipInfo(f"{FOLDER}/{name}")
                entry.create_system = 3
                mode = 0o755 if name == f"{APP}/{EXEC}" else 0o644
                entry.external_attr = (stat.S_IFREG | mode) << 16
                out.writestr(entry, (folder / name).read_bytes(), zipfile.ZIP_DEFLATED, 9)
        # macOS extraction must preserve the executable bit and signed resources.
        extracted = work / "Ensayo en ruta con espacios"
        subprocess.run(["ditto", "-x", "-k", str(archive), str(extracted)], check=True)
        require([p.name for p in extracted.iterdir()] == [FOLDER], "Raíz de ZIP inesperada")
        require(verify_folder(extracted / FOLDER, version) == inventory, "Extracción distinta")
        digest = sha(archive)
        checksum = work / (archive.name + ".sha256")
        checksum.write_text(f"{digest}  {archive.name}\n")
        manifest = work / "inventario-zip.json"
        manifest.write_text(json.dumps({"version": version, "perfil": "normal release arm64",
            "zip": archive.name, "sha256": digest, "archivos": inventory}, indent=2) + "\n")
        for f in (archive, checksum, manifest):
            os.replace(f, OUT / f.name)
        print(f"{archive.name}  {digest}")


if __name__ == "__main__":
    package()
