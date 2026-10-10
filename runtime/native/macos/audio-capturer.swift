// audio-capturer.swift
// Native audio capture helper for SuperCmd whisper dictation.
//
// Uses AVAudioEngine to capture microphone audio with minimal latency.
// Communicates via JSON-over-stdin/stdout (same pattern as whisper-transcriber serve mode).
//
// Commands (one JSON per line on stdout):
//   warmup     — start audio engine, emits {"ready":true}
//   start      — capture into ring buffer, emits {"recording":true}
//   stop       — write captured audio to WAV, emits {"file","duration"}
//   meter      — emits {"meter":{"average","peak"}}
//   drain      — emits {"pcm":"<base64 int16 LE>","dropped":N} appended since
//                previous drain; dropped>0 = ring overflow lost the head
//   stopEngine — mic cold, emits {"stopped":true}
//   ping/exit  — health check / clean shutdown
//
// File-source mode (e2e, no mic): if MUNDUS_DICTATION_CAPTURE_FILE is set, `start`
// decodes that file (wav/mp3/m4a via AVAudioFile) to 16 kHz mono and feeds the
// same ring/meter/drain path in ~50 ms blocks at real time / SPEED env (default 1.0),
// then silence until `stop`. Env is read at each `start` — swap files freely.

import Foundation
import AppKit
import AVFoundation

// KOS-376: bundled helper — without the accessory policy it would claim a
// second Dock tile under the Mundus Manager identity.
NSApplication.shared.setActivationPolicy(.accessory)

// MARK: - Constants

let targetSampleRate: Double = 16_000
// Safety net only — `drain` streams PCM out continuously, so recordings
// longer than the ring still reach the Engine intact.
let ringBufferDuration: TimeInterval = 60.0
let meterInterval: TimeInterval = 0.05

// MARK: - Ring Buffer

final class FloatRingBuffer {
  private let lock = NSLock()
  private var buffer: [Float]
  private var writeIndex = 0
  private var validSampleCount = 0

  init(capacity: Int) {
    buffer = Array(repeating: 0, count: max(1, capacity))
  }

  func append(_ samples: UnsafeBufferPointer<Float>) {
    guard !samples.isEmpty else { return }
    lock.lock()
    defer { lock.unlock() }
    for sample in samples {
      buffer[writeIndex] = sample
      writeIndex = (writeIndex + 1) % buffer.count
    }
    validSampleCount = min(buffer.count, validSampleCount + samples.count)
  }

  func recentSamples(count requestedCount: Int) -> [Float] {
    lock.lock()
    defer { lock.unlock() }
    let sampleCount = min(max(0, requestedCount), validSampleCount)
    guard sampleCount > 0 else { return [] }
    let startIndex = (writeIndex - sampleCount + buffer.count) % buffer.count
    if startIndex + sampleCount <= buffer.count {
      return Array(buffer[startIndex ..< startIndex + sampleCount])
    }
    let firstChunk = Array(buffer[startIndex ..< buffer.count])
    let secondChunk = Array(buffer[0 ..< (sampleCount - firstChunk.count)])
    return firstChunk + secondChunk
  }

  func sampleCount() -> Int {
    lock.lock()
    defer { lock.unlock() }
    return validSampleCount
  }

  func clear() {
    lock.lock()
    defer { lock.unlock() }
    writeIndex = 0
    validSampleCount = 0
  }
}

// MARK: - JSON output

func emitJSON(_ dict: [String: Any]) {
  guard let data = try? JSONSerialization.data(withJSONObject: dict),
        let line = String(data: data, encoding: .utf8)
  else { return }
  FileHandle.standardOutput.write(Data((line + "\n").utf8))
  fflush(stdout)
}

// MARK: - WAV writing

func writeWaveFile(samples: [Float], sampleRate: Double, toPath path: String) throws {
  let dataSize = UInt32(samples.count * 2)
  var data = Data(capacity: Int(44 + dataSize))
  func u32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { data.append(contentsOf: $0) } }
  func u16(_ v: UInt16) { withUnsafeBytes(of: v.littleEndian) { data.append(contentsOf: $0) } }

  data.append(contentsOf: "RIFF".utf8); u32(36 + dataSize)
  data.append(contentsOf: "WAVEfmt ".utf8)
  u32(16); u16(1); u16(1)                    // PCM, mono
  u32(UInt32(sampleRate)); u32(UInt32(sampleRate) * 2)
  u16(2); u16(16)                            // block align, bits
  data.append(contentsOf: "data".utf8); u32(dataSize)

  for sample in samples {
    var v = Int16(max(-1.0, min(1.0, sample)) * Float(Int16.max)).littleEndian
    withUnsafeBytes(of: &v) { data.append(contentsOf: $0) }
  }
  try data.write(to: URL(fileURLWithPath: path), options: .atomic)
}

// MARK: - Audio Capturer

class AudioCapturer {
  private var engine: AVAudioEngine?
  private var converter: AVAudioConverter?
  private let targetFormat = AVAudioFormat(
    commonFormat: .pcmFormatFloat32,
    sampleRate: targetSampleRate,
    channels: 1,
    interleaved: false
  )!
  private let ringBuffer = FloatRingBuffer(
    capacity: Int(targetSampleRate * ringBufferDuration)
  )
  private var isRecording = false
  private var isWarmingUp = false
  private var recordingStartedAt: Date?
  private var lastMeterAt: Date = Date.distantPast
  private var meterAverage: Double = 0
  private var meterPeak: Double = 0
  // Monotonic `drain` counters — ring overwrite between drains → dropped.
  private var totalWrittenSamples = 0
  private var drainedSamples = 0
  // File-source mode: paced feeder instead of the mic tap.
  private let feedQueue = DispatchQueue(label: "mundus.audio-capturer.feed")
  private var feedTimer: DispatchSourceTimer?
  /// Capture-file override path, read lazily so the file can swap between runs.
  private var captureFilePath: String? {
    let path = ProcessInfo.processInfo.environment["MUNDUS_DICTATION_CAPTURE_FILE"] ?? ""
    return path.isEmpty ? nil : path
  }

  // MARK: Engine lifecycle

  func startEngine() throws {
    if engine?.isRunning == true {
      emitJSON(["ready": true, "alreadyRunning": true])
      return
    }
    if captureFilePath != nil { // file-source mode: no mic/engine
      emitJSON(["ready": true, "fileSource": true])
      return
    }

    stopEngine()

    let engine = AVAudioEngine()
    let inputNode = engine.inputNode
    let inputFormat = inputNode.outputFormat(forBus: 0)

    guard let converter = AVAudioConverter(from: inputFormat, to: targetFormat) else {
      throw NSError(
        domain: "AudioCapturer",
        code: -1,
        userInfo: [NSLocalizedDescriptionKey: "Unable to create audio converter."]
      )
    }
    if inputFormat.channelCount > 1 {
      converter.channelMap = [NSNumber(value: 0)]
    }
    self.converter = converter

    inputNode.installTap(onBus: 0, bufferSize: 2048, format: inputFormat) { [weak self] buffer, _ in
      self?.processBuffer(buffer)
    }

    engine.prepare()
    try engine.start()
    self.engine = engine

    emitJSON(["ready": true])
  }

  func stopEngine() {
    stopFeeder()
    if let inputNode = engine?.inputNode {
      inputNode.removeTap(onBus: 0)
    }
    engine?.stop()
    engine = nil
    converter = nil
    isWarmingUp = false
    isRecording = false
  }

  var isEngineRunning: Bool {
    engine?.isRunning == true
  }

  // MARK: Recording

  func startRecording() {
    if let path = captureFilePath {
      startFileRecording(path: path)
      return
    }
    guard engine?.isRunning == true else {
      emitJSON(["error": "Audio engine not running. Call warmup first."])
      return
    }
    beginRecording()
  }

  private func beginRecording() {
    ringBuffer.clear()
    totalWrittenSamples = 0
    drainedSamples = 0
    isRecording = true
    recordingStartedAt = Date()
    emitJSON(["recording": true])
  }

  /// File-source capture: decode once, feed the ring in 50 ms blocks paced
  /// by SPEED env; silence past EOF until `stop`. `samples` stays local to
  /// the timer closure — no shared mutable state with stopFeeder.
  private func startFileRecording(path: String) {
    let samples: [Float]
    do {
      samples = try loadAudioFile(path)
    } catch {
      emitJSON(["error": "Cannot read capture file \(path): \(error.localizedDescription)"])
      return
    }
    beginRecording()
    let env = ProcessInfo.processInfo.environment
    let speed = max(0.01, Double(env["MUNDUS_DICTATION_CAPTURE_FILE_SPEED"] ?? "") ?? 1.0)
    let block = Int(targetSampleRate * 0.05)
    let timer = DispatchSource.makeTimerSource(queue: feedQueue)
    timer.schedule(deadline: .now(), repeating: 0.05 / speed)
    var pos = 0
    timer.setEventHandler { [weak self] in
      guard let self, self.isRecording else { return }
      if pos < samples.count {
        let end = min(pos + block, samples.count)
        samples.withUnsafeBufferPointer { base in
          self.ingest(UnsafeBufferPointer(start: base.baseAddress! + pos, count: end - pos))
        }
      } else {
        [Float](repeating: 0, count: block).withUnsafeBufferPointer { self.ingest($0) }
      }
      pos += block
    }
    timer.resume()
    feedTimer = timer
  }

  private func stopFeeder() {
    feedTimer?.cancel()
    feedTimer = nil
  }

  /// Декодировать файл в float-сэмплы 16 кГц mono — формат микрофонного tap'а.
  private func loadAudioFile(_ path: String) throws -> [Float] {
    let file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
    let inFormat = file.processingFormat
    guard let converter = AVAudioConverter(from: inFormat, to: targetFormat) else {
      throw NSError(domain: "AudioCapturer", code: -2, userInfo: [
        NSLocalizedDescriptionKey: "Unable to create audio converter.",
      ])
    }
    if inFormat.channelCount > 1 {
      converter.channelMap = [NSNumber(value: 0)]
    }
    let input = AVAudioPCMBuffer(
      pcmFormat: inFormat, frameCapacity: AVAudioFrameCount(file.length))!
    try file.read(into: input)
    let ratio = targetFormat.sampleRate / inFormat.sampleRate
    let capacity = AVAudioFrameCount((Double(input.frameLength) * ratio).rounded(.up) + 32)
    let output = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: capacity)!
    var error: NSError?
    var consumed = false
    _ = converter.convert(to: output, error: &error) { _, outStatus in
      defer { consumed = true }
      outStatus.pointee = consumed ? .endOfStream : .haveData
      return consumed ? nil : input
    }
    if let error { throw error }
    guard output.frameLength > 0, let channel = output.floatChannelData else { return [] }
    return Array(UnsafeBufferPointer(start: channel[0], count: Int(output.frameLength)))
  }

  /// All samples since the previous drain as base64 int16 LE; `dropped`
  /// reports a ring overflow (only the still-buffered tail is sent).
  func drain() {
    let pending = max(0, totalWrittenSamples - drainedSamples)
    let emit = min(pending, ringBuffer.sampleCount())
    let dropped = pending - emit
    var bytes = Data()
    if emit > 0 {
      let floats = ringBuffer.recentSamples(count: emit)
      bytes.reserveCapacity(emit * 2)
      for sample in floats {
        let clamped = max(-1.0, min(1.0, sample))
        var v = Int16(clamped * Float(Int16.max))
        withUnsafeBytes(of: &v) { bytes.append(contentsOf: $0) }
      }
    }
    drainedSamples = totalWrittenSamples
    emitJSON(["pcm": bytes.base64EncodedString(), "dropped": dropped])
  }

  func stopRecording() -> String? {
    guard isRecording else {
      emitJSON(["error": "Not recording"])
      return nil
    }
    stopFeeder()

    let samples = ringBuffer.recentSamples(count: ringBuffer.sampleCount())
    let duration = samples.count / Int(targetSampleRate)
    isRecording = false
    guard !samples.isEmpty else {
      emitJSON(["file": NSNull(), "duration": 0])
      return nil
    }

    let tempDir = FileManager.default.temporaryDirectory
      .appendingPathComponent("supercmd-audio-capture-\(UUID().uuidString)")
    let filePath = tempDir.path + "/captured.wav"

    do {
      try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
      try writeWaveFile(samples: samples, sampleRate: targetSampleRate, toPath: filePath)
      emitJSON(["file": filePath, "duration": duration])
      return filePath
    } catch {
      emitJSON(["error": "Failed to write WAV: \(error.localizedDescription)"])
      return nil
    }
  }

  // MARK: Buffer processing

  /// Единая точка входа сэмплов в ring/meter — mic tap и файловый feeder.
  private func ingest(_ samples: UnsafeBufferPointer<Float>) {
    let sampleCount = samples.count
    guard sampleCount > 0 else { return }

    if isRecording {
      ringBuffer.append(samples)
      totalWrittenSamples += sampleCount
    }

    // Compute meter
    let now = Date()
    if now.timeIntervalSince(lastMeterAt) >= meterInterval {
      var sumSquares: Float = 0
      var peak: Float = 0
      for i in 0..<sampleCount {
        let s = samples[i]
        sumSquares += s * s
        peak = max(peak, abs(s))
      }
      let rms = sqrt(sumSquares / Float(max(1, sampleCount)))
      meterAverage = Double(min(1, rms * 5))
      meterPeak = Double(min(1, peak * 5))
      lastMeterAt = now
    }
  }

  private func processBuffer(_ buffer: AVAudioPCMBuffer) {
    guard let converted = convertBuffer(buffer),
          converted.frameLength > 0,
          let samples = converted.floatChannelData?[0]
    else { return }
    ingest(UnsafeBufferPointer(start: samples, count: Int(converted.frameLength)))
  }

  func getMeter() -> [String: Double] {
    return ["average": meterAverage, "peak": meterPeak]
  }

  // MARK: Audio conversion

  private func convertBuffer(_ inputBuffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
    guard let converter else { return nil }

    let sampleRateRatio = targetFormat.sampleRate / inputBuffer.format.sampleRate
    let frameCapacity = AVAudioFrameCount(
      max(1, (Double(inputBuffer.frameLength) * sampleRateRatio).rounded(.up) + 32)
    )

    guard let outputBuffer = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: frameCapacity) else {
      return nil
    }

    var error: NSError?
    var consumedInput = false
    let status = converter.convert(to: outputBuffer, error: &error) { _, outStatus in
      if consumedInput {
        outStatus.pointee = .noDataNow
        return nil
      }
      consumedInput = true
      outStatus.pointee = .haveData
      return inputBuffer
    }

    if error != nil {
      return nil
    }

    switch status {
    case .haveData, .inputRanDry, .endOfStream:
      return outputBuffer.frameLength > 0 ? outputBuffer : nil
    case .error:
      return nil
    @unknown default:
      return nil
    }
  }

  // MARK: Cleanup

  func cleanup() {
    stopEngine()
    ringBuffer.clear()
  }
}

// MARK: - Main loop

let capturer = AudioCapturer()

// Clean shutdown on signals
for sig in [SIGINT, SIGTERM] {
  let source = DispatchSource.makeSignalSource(signal: sig, queue: .main)
  source.setEventHandler {
    capturer.cleanup()
    exit(0)
  }
  source.resume()
  signal(sig, SIG_IGN)
}

// Commands loop: one JSON per stdin line.
while let line = readLine(strippingNewline: true) {
  let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
  guard !trimmed.isEmpty,
        let data = trimmed.data(using: .utf8),
        let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
  else {
    if !trimmed.isEmpty { emitJSON(["error": "Invalid JSON request"]) }
    continue
  }
  let command = json["command"] as? String ?? ""

  switch command {
  case "warmup":
    do {
      try capturer.startEngine()
    } catch {
      emitJSON(["error": "Failed to start audio engine: \(error.localizedDescription)"])
    }

  case "start":
    capturer.startRecording()

  case "stop":
    _ = capturer.stopRecording()

  case "meter":
    let m = capturer.getMeter()
    emitJSON(["meter": m])

  case "drain":
    capturer.drain()

  case "stopEngine":
    capturer.stopEngine()
    emitJSON(["stopped": true])

  case "ping":
    emitJSON(["pong": true])

  case "exit":
    capturer.cleanup()
    exit(0)

  default:
    emitJSON(["error": "Unknown command: \(command)"])
  }
}

capturer.cleanup()
