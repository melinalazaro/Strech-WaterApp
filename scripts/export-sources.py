"""Export reviewed source inputs only. Keep private development and user state out."""
import hashlib
import json
import os
import re
import shutil
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "entregas" / ("v" + json.loads((ROOT / "package.json").read_text())["version"])
GROUPS = {
    "": [".gitignore", ".stretch-water-project", "package.json", "package-lock.json",
         "index.html", "tsconfig.json", "vite.config.ts"],
    "src": ["main.tsx", "styles.css", "media.ts", "catalog.ts", "vite-env.d.ts"],
    "src/assets": ["placeholder.svg", "stretch-poses.svg", "water-poses.svg"],
    "src-tauri": ["Cargo.toml", "Cargo.lock", "build.rs", "Info.plist",
                  "tauri.conf.json", "tauri.test.conf.json"],
    "src-tauri/src": ["main.rs", "audio.rs", "coordinator.rs", "logging.rs", "login.rs", "menu.rs",
                      "phase3_tests.rs", "power.rs", "preferences.rs", "presentation.rs",
                      "runtime.rs", "scheduler.rs"],
    "src-tauri/native": ["audio.m", "login.m", "power.m", "reminder.m", "reminder_frames.h", "reminder_frames.m"],
    "src-tauri/assets": ["cling.wav", "reserva.png"],
    "src-tauri/icons": ["lotus.icns", "lotus.png", "lotus-tray.rgba", "icon.png"],
    "tests": ["media.test.mjs", "native-reminder.m", "reminder-position.m", "reminder-screens.m"],
    "scripts": ["generate-provisional-media.py", "build-release.sh", "package-release.py", "export-sources.py"],
    "docs": ["LEEME-INSTALACION.txt", "README-FUENTES.md", "VALIDACION-0.5.1.md"],
}


def export():
    files = {Path(group)/name: Path(group)/name for group, names in GROUPS.items() for name in names}
    for kind in ("stretch", "water"):
        directory = Path("src-tauri/assets/films") / kind
        for name in ["durations.json", *(f"{i:03d}.png" for i in range(241))]:
            files[directory / name] = directory / name
    files[Path("README.md")] = Path("docs/README-FUENTES.md")
    OUT.mkdir(exist_ok=True, parents=True)
    with TemporaryDirectory(dir=OUT, prefix=".fuentes-") as tmp:
        copy = Path(tmp)/"fuentes"
        hashes = []
        for dst, src in sorted(files.items()):
            origin = ROOT/src
            if origin.resolve() != origin or not origin.is_file():
                raise ValueError(f"Fuente no válida: {src}")
            target = copy/dst
            target.parent.mkdir(exist_ok=True, parents=True)
            shutil.copyfile(origin, target)
            blob = target.read_bytes()
            if os.fsencode(ROOT) in blob or re.search(rb"/Users/[\w.-]+/|-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----", blob):
                raise ValueError(f"Contenido privado inesperado: {src}")
            hashes.append(f"{hashlib.sha256(blob).hexdigest()}  {dst.as_posix()}")
        inventory = Path(tmp)/"inventario-fuentes.sha256"
        inventory.write_text("\n".join(hashes)+"\n")
        dest = OUT/"fuentes"
        if dest.exists():
            raise FileExistsError("Ya existe fuentes. Conservá tu copia y exportá a una carpeta limpia.")
        copy.rename(dest)
        os.replace(inventory, OUT/inventory.name)
        print(f"{len(hashes)} archivos exportados a fuentes; inventario SHA-256 preparado.")


if __name__ == "__main__":
    export()
