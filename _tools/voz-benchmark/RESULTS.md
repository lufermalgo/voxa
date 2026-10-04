# Voz / Whisper: lokaler Vorabtest

Gemessen am 4. Oktober 2026 auf Apple M1 Pro, 16 GB RAM, macOS 26.6.2.

**Voz war in diesem synthetischen Test schneller und hatte weniger Wortfehler.**
Das rechtfertigt einen Test mit echten Diktaten, aber noch keine Umstellung von Voxa.

| Messwert | Voxas Whisper Small | Voz |
|---|---:|---:|
| Mittlere Transkriptionszeit (Median) | 0.537 s | 0.241 s |
| 95. Perzentil der Transkriptionszeit | 2.034 s | 0.522 s |
| Wortfehlerrate auf synthetischem Korpus | 8.17 % | 2.43 % |
| Erster Modellstart nach Download | 0,30 s | **55,27 s** |
| Weitere Modellstarts (zwei frische Prozesse) | 0,27–0,35 s | 0,29–0,40 s |
| Erstes Diktat nach 60 Sekunden Pause, Modell geladen | 0,86 s | 0,23 s |

Vozs erste Initialisierung war wesentlich länger als die grobe Herstellerangabe.
Sie umfasst die erstmalige Core-ML-/Neural-Engine-Spezialisierung; welcher Anteil
auf welchen internen Schritt entfällt, ist nicht getrennt gemessen. Dieser Start
muss bei einer Integration während der Einrichtung abgefangen werden.
Die Pause wurde einmal pro Engine mit demselben kurzen Sample gemessen;
sie reicht nicht für eine belastbare Aussage über längere Inaktivität.

## Unterschied nach Aufnahmelänge

Median bei geladenem Modell, jeweils 10 Dateien × 3 Durchgänge:

| Tatsächliche Länge | Whisper | Voz |
|---|---:|---:|
| 2,1–3,4 s | 0.359 s | 0.115 s |
| 7,7–9,0 s | 0.537 s | 0.241 s |
| 37,1–38,4 s | 1.827 s | 0.509 s |

## Speicherbeobachtung

Höchster beobachteter Prozess-RSS: Whisper 713–729 MiB; Voz beim ersten
Modellstart 528 MiB, in späteren Durchgängen 116–118 MiB. Diese Werte sind keine
vollständige Messung des RAM-Bedarfs: GPU-/Neural-Engine-Allokationen können außerhalb
des Prozess-RSS liegen. Abtastung alle 100 ms; kurze Spitzen können fehlen.
Kein belastbarer Vergleich des gesamten Voxa-Speicherbedarfs.

## Verfahren und Grenzen

- 30 synthetische Dateien: 15 von macOS Anna gesprochene Texte, jeweils sauber
  und mit künstlichem rosa Rauschen. Drei Grundtexte mit kleinen Suffixvarianten.
  Das sind keine 30 unabhängigen menschlichen Diktate und keine echte Raumakustik.
- Drei Durchgänge pro Engine, jeweils allein, Reihenfolge wechselnd. 90 warme
  Messungen pro Engine; Wiederholungen zählen nicht als zusätzliche Aufnahmen.
- Identische mono 16-kHz-Float32-Daten; Umwandlung und Dateiladen außerhalb der
  Transkriptionszeit. Beide Modelle bleiben innerhalb eines Durchgangs geladen.
- Whisper verwendet die unveränderte Produktionsimplementierung mit Metal,
  vier Threads, Deutsch, Greedy best-of-one, leerem Vokabular-Prompt sowie Voxas
  Halluzinationsfilter und Speicher-Reset. Gemessen wird der normale Batch-Pfad;
  die optionale Fensterverarbeitung wurde nicht verglichen.
- Voz: stabile SDK-Version 3.5.0, Modellrevision v0.1.0. Das ist kein Nachweis
  der Leistungswerte für neuere SDK-/Modellversionen auf der Herstellerwebsite.
- Wortfehlerrate mit NFC, Kleinschreibung und entfernter Interpunktion;
  Zahlen in Ziffern und Worten werden nicht gleichgesetzt. Referenztexte sind
  TTS-Eingaben; ihre Übereinstimmung mit der gesprochenen Ausgabe ist nicht
  manuell auditiert. Einzelne Fehler betreffen Namen und Voxa.
- Modellstart ist ein neuer Prozess, kein erzwungen leerer Betriebssystemcache.
- Nicht gemessen: LLM-Nachbearbeitung, endgültiger Text, Mikrofon-Endverarbeitung,
  Hotkey-Loslassen bis tatsächlich eingefügter Text und gesamte App-Speichernutzung.

## Nächster Schritt

30 echte deutsche Aufnahmen mit überprüften Referenztexten vergleichen. Wenn
Geschwindigkeit und Qualität bestehen, Voz in einem gesonderten Integrationsschritt
an Voxa anbinden und mit identischen LLM-/Profil-Einstellungen die Zeit bis zum
sichtbaren Text in einer kontrollierten Ziel-App messen. Keine Summe einzelner
Medianwerte als angeblich gemessene Gesamtzeit ausgeben.

## Lokale Belege

- `results/measured/summary.json`: zusammengefasste Messwerte und Modellstarts.
- `results/measured/runs.jsonl`: 188 einzelne Messungen und Transkripte.
- `results/measured/metadata.json`: Audio-Hashes, Hardware, Korpus und Einstellungen.
- `results/measured/*.log`: Ausgaben der beiden Engines.
- `README.md`: Wiederholung mit eigenem Korpus.

Swift-Release-Build, Rust-Release-Build, `cargo check` des isolierten Benchmark-Pakets
und vier Tests der Wortfehler-Auswertung erfolgreich. Produktcode unverändert.
Audiodateien, Modelle und ausführliche Rohdaten bleiben lokal und sind gitignoriert.
