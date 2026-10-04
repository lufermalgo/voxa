import Foundation
import Voz

func emit(_ fields: [String: Any]) throws {
    let data = try JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}

@main struct Bench {
    static func main() async throws {
        guard CommandLine.arguments.count >= 2 else {
            throw NSError(domain: "VozBench", code: 1, userInfo: [NSLocalizedDescriptionKey: "model directory required"])
        }
        let directory = CommandLine.arguments[1]
        if CommandLine.arguments.contains("--download") {
            _ = try await Voz.download(directory: directory)
            try emit(["downloaded": true])
            return
        }
        let clock = ContinuousClock()
        let start = clock.now
        let voz = try Voz(modelDirectory: URL(fileURLWithPath: directory))
        guard voz.sampleRate == 16_000 else {
            throw NSError(domain: "VozBench", code: 1, userInfo: [NSLocalizedDescriptionKey: "Expected 16 kHz model"])
        }
        try emit(["ready": true, "load_ms": milliseconds(start.duration(to: clock.now))])
        while let line = readLine() {
            do {
                guard let request = try JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: String],
                      let path = request["pcm"] else {
                    throw NSError(domain: "VozBench", code: 2)
                }
                let bytes = try Data(contentsOf: URL(fileURLWithPath: path))
                guard !bytes.isEmpty && bytes.count % 4 == 0 else {
                    throw NSError(domain: "VozBench", code: 2)
                }
                let samples: [Float] = bytes.withUnsafeBytes { buffer in
                    stride(from: 0, to: bytes.count, by: 4).map {
                        Float(bitPattern: UInt32(littleEndian: buffer.loadUnaligned(fromByteOffset: $0, as: UInt32.self)))
                    }
                }
                let start = clock.now
                let result = try await voz.transcribe(samples: samples)
                try emit(["text": result.text, "stt_ms": milliseconds(start.duration(to: clock.now))])
            } catch { try emit(["error": String(describing: error)]) }
        }
    }
    static func milliseconds(_ d: Duration) -> Double {
        Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
    }
}
