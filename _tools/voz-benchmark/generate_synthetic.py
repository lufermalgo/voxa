"""Generate synthetic smoke-test audio, not evidence of human dictation accuracy."""
import json
import subprocess
from pathlib import Path

TEXTS = ['Bitte schicke mir morgen die Unterlagen.', 'Der Termin mit Jörg Müller ist am Donnerstag um halb zehn. Bitte bring die aktuellen Unterlagen und den unterschriebenen Vertrag mit.', 'Bitte fasse das Gespräch mit Jörg Müller zusammen. Wir haben vereinbart, dass die neue Version von Voxa am Donnerstag getestet wird. Dabei sollen deutsche Diktate, Namen und technische Begriffe berücksichtigt werden. Prüfe außerdem, ob kurze Aufnahmen zuverlässig verarbeitet werden und ob nach einer längeren Pause zusätzliche Wartezeit entsteht. Die Ergebnisse brauchen wir für die Entscheidung, welche Spracherkennung wir künftig verwenden. Schicke mir anschließend eine kurze Übersicht mit den wichtigsten Fehlern und den gemessenen Zeiten. Wenn etwas unklar ist, dokumentiere es bitte ausdrücklich.']

def main():
    root = Path(__file__).resolve().parent / 'data'
    root.mkdir(exist_ok=True)
    manifest = []
    for group, text in enumerate(TEXTS):
        for index, suffix in enumerate(['', ' Bitte heute.', ' Danke dafür.', ' Das ist wichtig.', ' Ich warte darauf.']):
            reference = text + suffix
            identifier = f'g{group+1}-{index+1}'
            aiff = root / f'{identifier}.aiff'
            clean = root / f'{identifier}-clean.wav'
            subprocess.run(['say', '-v', 'Anna', '-r', '175', '-o', str(aiff), reference], check=True)
            subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', str(aiff), '-ac', '1', '-ar', '16000', str(clean)], check=True)
            noisy = root / f'{identifier}-noise.wav'
            subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', str(clean), '-f', 'lavfi', '-i', f'anoisesrc=color=pink:amplitude=0.03:seed={42+index}', '-filter_complex', '[0:a][1:a]amix=inputs=2:duration=first:normalize=0', '-ac', '1', '-ar', '16000', str(noisy)], check=True)
            for condition in ['clean', 'noise']:
                manifest.append(dict(id=f'{identifier}-{condition}', audio=f'{identifier}-{condition}.wav', reference=reference,
                    source_kind='synthetic', condition=condition, target_duration_group=[3,10,30][group],
                    voice='macOS Anna, 175 words/minute', noise='none' if condition == 'clean' else 'artificial pink noise, amplitude 0.03'))
    (root / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2))
    print(f'Created {len(manifest)} synthetic samples; references require auditing against generated audio')

if __name__ == '__main__':
    main()
