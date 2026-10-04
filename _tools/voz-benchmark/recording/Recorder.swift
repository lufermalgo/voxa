import AppKit
import AVFoundation

struct Prompt: Codable {
    let id: String
    let title: String
    let reference: String
    let expected_final: String
}

@MainActor final class Recorder: NSObject, NSApplicationDelegate, NSWindowDelegate, AVAudioRecorderDelegate {
    var window: NSWindow!
    var prompts: [Prompt] = []
    var index = 0
    var recorder: AVAudioRecorder?
    var timer: Timer?
    var pendingURL: URL?
    var recordings: [[String: Any]] = []
    var root: URL!
    let titleLabel = NSTextField(labelWithString: "")
    let text = NSTextView()
    let status = NSTextField(labelWithString: "Bereit. Es wird noch nichts aufgenommen.")
    let recordButton = NSButton(title: "Aufnahme starten", target: nil, action: nil)
    let nextButton = NSButton(title: "Nächster Text", target: nil, action: nil)
    let previousButton = NSButton(title: "Zurück", target: nil, action: nil)

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            root = Bundle.main.bundleURL.deletingLastPathComponent()
            let resource = Bundle.main.url(forResource: "prompts", withExtension: "json")!
            prompts = try JSONDecoder().decode([Prompt].self, from: Data(contentsOf: resource))
            let manifest = root.appendingPathComponent("manifest.json")
            if FileManager.default.fileExists(atPath: manifest.path) {
                recordings = (try JSONSerialization.jsonObject(with: Data(contentsOf: manifest)) as? [[String: Any]]) ?? []
            }
            try FileManager.default.createDirectory(at: root.appendingPathComponent("Aufnahmen"), withIntermediateDirectories: true)
            buildWindow()
        } catch { showError(error.localizedDescription); NSApp.terminate(nil) }
    }

    func buildWindow() {
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 560),
                          styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
        window.title = "Voxa · Sprachtest"
        window.delegate = self
        let view = window.contentView!
        titleLabel.frame = NSRect(x: 28, y: 493, width: 704, height: 32)
        titleLabel.font = .systemFont(ofSize: 24, weight: .semibold)
        view.addSubview(titleLabel)
        let hint = NSTextField(wrappingLabelWithString: "Natürlich vorlesen, mit deinem üblichen Mikrofon. Start drücken, sprechen, Stop drücken. Dateien werden automatisch gespeichert. Wenn du dich versprichst, kannst du denselben Text noch einmal aufnehmen.")
        hint.frame = NSRect(x: 28, y: 416, width: 704, height: 65)
        hint.font = .systemFont(ofSize: 14)
        view.addSubview(hint)
        let scroll = NSScrollView(frame: NSRect(x: 28, y: 150, width: 704, height: 248))
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        text.frame = scroll.bounds
        text.isEditable = false
        text.isSelectable = true
        text.font = .systemFont(ofSize: 21)
        text.textContainerInset = NSSize(width: 14, height: 14)
        text.isVerticallyResizable = true
        text.autoresizingMask = [.width]
        text.textContainer?.widthTracksTextView = true
        scroll.documentView = text
        view.addSubview(scroll)
        status.frame = NSRect(x: 28, y: 103, width: 704, height: 30)
        status.font = .monospacedDigitSystemFont(ofSize: 15, weight: .medium)
        view.addSubview(status)
        for (button, x, width, action) in [
            (previousButton, CGFloat(28), CGFloat(90), #selector(previous)),
            (recordButton, CGFloat(137), CGFloat(205), #selector(toggle)),
            (nextButton, CGFloat(360), CGFloat(145), #selector(next))
        ] {
            button.frame = NSRect(x: x, y: 42, width: width, height: 38)
            button.bezelStyle = .rounded
            button.target = self
            button.action = action
            view.addSubview(button)
        }
        let folder = NSButton(title: "Ordner öffnen", target: self, action: #selector(openFolder))
        folder.frame = NSRect(x: 580, y: 42, width: 152, height: 38)
        folder.bezelStyle = .rounded
        view.addSubview(folder)
        let menu = NSMenu()
        let item = NSMenuItem()
        let submenu = NSMenu()
        submenu.addItem(withTitle: "Sprachtest beenden", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.submenu = submenu
        menu.addItem(item)
        NSApp.mainMenu = menu
        display()
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func display() {
        let prompt = prompts[index]
        titleLabel.stringValue = prompt.title
        text.string = prompt.reference
        previousButton.isEnabled = index > 0
        nextButton.isEnabled = index < prompts.count - 1 && completed(prompt.id)
        recordButton.title = completed(prompt.id) ? "Erneut aufnehmen" : "Aufnahme starten"
        status.textColor = .labelColor
        status.stringValue = completed(prompt.id) ? "Gespeichert. \(recordings.count) von \(prompts.count) Texten aufgenommen." : "Bereit. Es wird noch nichts aufgenommen."
    }

    func completed(_ id: String) -> Bool { recordings.contains { $0["id"] as? String == id } }
    @objc func previous() { guard recorder == nil, index > 0 else { return }; index -= 1; display() }
    @objc func next() { guard recorder == nil, index < prompts.count - 1 else { return }; index += 1; display() }
    @objc func openFolder() { NSWorkspace.shared.open(root) }

    @objc func toggle() {
        if recorder != nil { finish(); return }
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: start()
        case .notDetermined:
            recordButton.isEnabled = false
            AVCaptureDevice.requestAccess(for: .audio) { [weak self] granted in
                Task { @MainActor in
                    guard let self else { return }
                    self.recordButton.isEnabled = true
                    if granted { self.start() } else { self.showError("Mikrofonzugriff wurde nicht erlaubt. Freigabe unter Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon.") }
                }
            }
        default: showError("Bitte erlaube Sprachtest den Mikrofonzugriff unter Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon.")
        }
    }

    func start() {
        do {
            let name = "\(prompts[index].id)-\(UUID().uuidString.prefix(8)).wav"
            let url = root.appendingPathComponent("Aufnahmen").appendingPathComponent(name)
            let settings: [String: Any] = [AVFormatIDKey: kAudioFormatLinearPCM,
                AVSampleRateKey: 48000.0, AVNumberOfChannelsKey: 1,
                AVLinearPCMBitDepthKey: 16, AVLinearPCMIsFloatKey: false,
                AVLinearPCMIsBigEndianKey: false]
            let audio = try AVAudioRecorder(url: url, settings: settings)
            audio.delegate = self
            guard audio.prepareToRecord(), audio.record() else { throw NSError(domain: "Recorder", code: 1, userInfo: [NSLocalizedDescriptionKey: "Aufnahme konnte nicht starten."]) }
            recorder = audio
            pendingURL = url
            previousButton.isEnabled = false
            nextButton.isEnabled = false
            recordButton.title = "Stop · Speichern"
            status.textColor = .systemRed
            status.stringValue = "● Aufnahme läuft · 0 Sekunden"
            timer = Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { [weak self] _ in
                Task { @MainActor in
                    guard let self, let recorder = self.recorder else { return }
                    self.status.stringValue = "● Aufnahme läuft · \(Int(recorder.currentTime)) Sekunden"
                }
            }
        } catch { showError(error.localizedDescription) }
    }

    func finish() {
        guard let audio = recorder, let url = pendingURL else { return }
        let duration = audio.currentTime
        audio.stop()
        recorder = nil
        timer?.invalidate()
        timer = nil
        do {
            guard duration >= 0.5 else { throw NSError(domain: "Recorder", code: 2, userInfo: [NSLocalizedDescriptionKey: "Aufnahme war zu kurz. Bitte erneut aufnehmen."]) }
            let p = prompts[index]
            let entry: [String: Any] = ["id": p.id, "audio": "Aufnahmen/\(url.lastPathComponent)",
                "reference": p.reference, "source_kind": "human", "condition": "user_normal_environment",
                "reference_status": "read_prompt_unverified", "expected_final": p.expected_final,
                "recorded_duration_s": duration, "recorded_at": ISO8601DateFormatter().string(from: Date())]
            var updated = recordings.filter { $0["id"] as? String != p.id }
            updated.append(entry)
            updated.sort { ($0["id"] as! String) < ($1["id"] as! String) }
            let data = try JSONSerialization.data(withJSONObject: updated, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: root.appendingPathComponent("manifest.json"), options: .atomic)
            recordings = updated
            display()
            if recordings.count == prompts.count {
                status.stringValue = "Alle sechs Texte gespeichert. Schreib im Chat einfach: fertig."
            }
        } catch { display(); showError("\(error.localizedDescription) Die Audiodatei bleibt im Aufnahmeordner erhalten.") }
    }

    func windowWillClose(_ notification: Notification) { if recorder != nil { finish() }; NSApp.terminate(nil) }
    func applicationWillTerminate(_ notification: Notification) { if recorder != nil { finish() } }
    func showError(_ message: String) {
        let alert = NSAlert()
        alert.messageText = "Sprachtest"
        alert.informativeText = message
        alert.runModal()
    }
}

@main struct Main {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        let delegate = Recorder()
        app.delegate = delegate
        withExtendedLifetime(delegate) { app.run() }
    }
}
