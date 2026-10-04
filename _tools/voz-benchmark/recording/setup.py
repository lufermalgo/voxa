"""Build the local guided recorder and prepare its private recording folder."""
import json
import argparse
import plistlib
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare-only", action="store_true", help="write instructions without touching a running app")
    args = parser.parse_args()
    source = Path(__file__).resolve().parent
    root = source.parent / "data" / "Sprachtest"
    root.mkdir(parents=True, exist_ok=True)
    (root / "Aufnahmen").mkdir(exist_ok=True)
    app = root / "Sprachtest.app" / "Contents"
    (app / "MacOS").mkdir(parents=True, exist_ok=True)
    (app / "Resources").mkdir(exist_ok=True)
    prompts = json.loads((source / "prompts.json").read_text())
    if not args.prepare_only:
        (app / "Resources" / "prompts.json").write_text(json.dumps(prompts, ensure_ascii=False, indent=2))
        info = {
            "CFBundleExecutable": "Sprachtest", "CFBundleName": "Sprachtest",
            "CFBundleIdentifier": "com.voxa.benchmark.recorder", "CFBundleVersion": "1",
            "CFBundleShortVersionString": "1.0", "CFBundlePackageType": "APPL",
            "NSPrincipalClass": "NSApplication", "LSMinimumSystemVersion": "15.0",
            "NSMicrophoneUsageDescription": "Nimmt die von dir gestarteten Sprachtests auf und speichert sie lokal im Testordner.",
            "NSHighResolutionCapable": True,
        }
        (app / "Info.plist").write_bytes(plistlib.dumps(info))
        subprocess.run(["swiftc", "-parse-as-library", "-O", "-framework", "AppKit", "-framework", "AVFoundation",
                        str(source / "Recorder.swift"), "-o", str(app / "MacOS" / "Sprachtest")], check=True)
        # Finder metadata may be added to generated bundles inside an iCloud folder.
        # Remove only attributes codesign rejects; retain quarantine/security metadata.
        for path in [app.parent, *app.parent.rglob("*")]:
            attributes = subprocess.check_output(["xattr", str(path)], text=True).splitlines()
            for key in ("com.apple.FinderInfo", "com.apple.ResourceFork"):
                if key in attributes:
                    subprocess.run(["xattr", "-d", key, str(path)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", str(app.parent)], check=True)
    lines = ["Voxa-Sprachtest – sechs Aufnahmen, etwa zwei bis drei Minuten", "",
        "1. Sprachtest.app öffnen. Beim ersten Start einer Aufnahme Mikrofonzugriff erlauben.",
        "2. Aufnahme starten, angezeigten Text natürlich vorlesen, Stop drücken.",
        "3. Nächster Text anklicken. Bei einem Versprecher erneut aufnehmen.",
        "4. Nach sechs Texten im Chat schreiben: fertig. Speichern und Benennen erfolgen automatisch.", "",
        "Verwende dein übliches Mikrofon und eine normale Arbeitsumgebung. Keine absichtlichen Störgeräusche nötig.",
        "Die Texte sind Ausgangsreferenzen. Wenn du abweichst und nicht neu aufnimmst, sag es dazu; wir passen die Referenz an.",
        "Zahlen darf die Erkennung als Ziffern schreiben. Das wird zusätzlich inhaltlich beurteilt.",
        "Wir vergleichen zuerst Voz und Whisper OHNE LLM. Für den LLM-Vergleich verwenden wir dieselben Aufnahmen.", "",
        "Falls die App nicht funktioniert: QuickTime Player → Ablage → Neue Audioaufnahme.",
        "Mikrofon wählen, aufnehmen und pro Text in Aufnahmen speichern (z. B. 01-kurz.m4a).",
        "QuickTime-Dateien importiert der Assistent; keine JSON-Datei selbst bearbeiten.",
        "Apple-Anleitung: https://support.apple.com/guide/quicktime-player/record-audio-qtpf25d6f827/mac", "",
        "Texte:", ""]
    for prompt in prompts:
        lines += [prompt["title"], "Dateiname für QuickTime: " + prompt["id"] + ".m4a", prompt["reference"], ""]
    (root / "BITTE ZUERST LESEN.txt").write_text("\n".join(lines))
    (root / "Vorlesetexte.txt").write_text("\n\n".join(p["title"] + "\n" + p["reference"] for p in prompts))
    # Do not overwrite a user's already recorded corpus.
    manifest = root / "manifest.json"
    if not manifest.exists():
        manifest.write_text("[]\n")
    print(root)


if __name__ == "__main__":
    main()
