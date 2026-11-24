import Foundation
import Darwin

enum EngineError: LocalizedError {
    case missingEngineBinary
    case commandFailed(String)
    case decodeFailed(String)

    var errorDescription: String? {
        switch self {
        case .missingEngineBinary:
            return "未找到应用内置的 aicross 引擎可执行文件。"
        case .commandFailed(let msg):
            return msg.isEmpty ? "引擎执行失败。" : msg
        case .decodeFailed(let detail):
            return "引擎返回数据无法解析：\(detail)"
        }
    }
}

final class EngineClient: Sendable {
    static let shared = EngineClient()

    private final class EnvCache: @unchecked Sendable {
        private let lock = NSLock()
        private var cachedEnv: [String: String]?

        func get() -> [String: String]? {
            lock.lock()
            defer { lock.unlock() }
            return cachedEnv
        }

        func set(_ env: [String: String]) {
            lock.lock()
            cachedEnv = env
            lock.unlock()
        }

        func clear() {
            lock.lock()
            cachedEnv = nil
            lock.unlock()
        }
    }

    private let envCache = EnvCache()

    func invalidateEnvironmentCache() {
        envCache.clear()
    }

    func engineExecutableURL() -> URL? {
        let fm = FileManager.default
        var candidates: [URL] = []
        if let res = Bundle.main.resourceURL {
            candidates.append(res.appendingPathComponent("aicross-engine"))
        }
        if let exec = Bundle.main.executableURL {
            candidates.append(
                exec.deletingLastPathComponent()
                    .appendingPathComponent("../Resources/aicross-engine")
                    .standardizedFileURL
            )
        }
        for url in candidates where fm.isExecutableFile(atPath: url.path) {
            return url
        }
        return nil
    }

    func call<T: Decodable>(
        _ method: String,
        payload: [String: Any] = [:],
        timeout: TimeInterval = 180,
        onProgress: (@Sendable (DetectProgress) -> Void)? = nil
    ) async throws -> T {
        let data = try await callRaw(method, payload: payload, timeout: timeout, onProgress: onProgress)
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw EngineError.decodeFailed("\(method): \(error.localizedDescription)")
        }
    }

    @discardableResult
    func callVoid(
        _ method: String,
        payload: [String: Any] = [:],
        timeout: TimeInterval = 180
    ) async throws -> Data {
        try await callRaw(method, payload: payload, timeout: timeout, onProgress: nil)
    }

    func callRaw(
        _ method: String,
        payload: [String: Any] = [:],
        timeout: TimeInterval = 180,
        onProgress: (@Sendable (DetectProgress) -> Void)? = nil
    ) async throws -> Data {
        guard let engineURL = engineExecutableURL() else {
            throw EngineError.missingEngineBinary
        }

        let payloadString: String
        if payload.isEmpty {
            payloadString = "{}"
        } else {
            let jsonData = try JSONSerialization.data(withJSONObject: payload, options: [])
            payloadString = String(decoding: jsonData, as: UTF8.self)
        }

        let env = await resolvedEnvironment()

        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let result = try Self.executeProcess(
                        engineURL: engineURL,
                        method: method,
                        payloadString: payloadString,
                        env: env,
                        timeout: timeout,
                        onProgress: onProgress
                    )
                    continuation.resume(returning: result)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func executeProcess(
        engineURL: URL,
        method: String,
        payloadString: String,
        env: [String: String],
        timeout: TimeInterval,
        onProgress: (@Sendable (DetectProgress) -> Void)?
    ) throws -> Data {
        let process = Process()
        process.executableURL = engineURL
        process.arguments = ["__ui", method, payloadString]
        process.environment = env
        process.standardInput = FileHandle.nullDevice

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        final class BufferBox: @unchecked Sendable {
            let lock = NSLock()
            var data = Data()
            var linesRemainder = ""

            func append(_ chunk: Data) {
                lock.lock()
                data.append(chunk)
                lock.unlock()
            }

            func appendStderrChunk(_ chunk: Data, onProgress: (@Sendable (DetectProgress) -> Void)?) {
                guard !chunk.isEmpty else { return }
                let text = String(decoding: chunk, as: UTF8.self)
                lock.lock()
                let combined = linesRemainder + text
                var parts = combined.components(separatedBy: "\n")
                linesRemainder = parts.popLast() ?? ""
                var nonProgressLines: [String] = []
                for line in parts {
                    let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
                    if trimmed.hasPrefix("AICROSS_PROGRESS:") {
                        let jsonPart = String(trimmed.dropFirst("AICROSS_PROGRESS:".count))
                        if let pData = jsonPart.data(using: .utf8),
                           let prog = try? JSONDecoder().decode(DetectProgress.self, from: pData) {
                            lock.unlock()
                            onProgress?(prog)
                            lock.lock()
                        }
                    } else if !trimmed.isEmpty {
                        nonProgressLines.append(line)
                    }
                }
                if !nonProgressLines.isEmpty {
                    if !data.isEmpty { data.append(0x0A) }
                    data.append(Data(nonProgressLines.joined(separator: "\n").utf8))
                }
                lock.unlock()
            }

            func finalizeStderr() -> String {
                lock.lock()
                defer { lock.unlock() }
                var out = String(decoding: data, as: UTF8.self)
                let rem = linesRemainder.trimmingCharacters(in: .whitespacesAndNewlines)
                if !rem.isEmpty && !rem.hasPrefix("AICROSS_PROGRESS:") {
                    out = out.isEmpty ? rem : "\(out)\n\(rem)"
                }
                return out.trimmingCharacters(in: .whitespacesAndNewlines)
            }

            func getData() -> Data {
                lock.lock()
                defer { lock.unlock() }
                return data
            }
        }

        let outBox = BufferBox()
        let errBox = BufferBox()

        stderrPipe.fileHandleForReading.readabilityHandler = { handle in
            let chunk = handle.availableData
            if !chunk.isEmpty {
                errBox.appendStderrChunk(chunk, onProgress: onProgress)
            }
        }

        let readGroup = DispatchGroup()
        readGroup.enter()
        DispatchQueue.global(qos: .userInitiated).async {
            let allOut = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
            outBox.append(allOut)
            readGroup.leave()
        }

        try process.run()

        let deadline = Date().addingTimeInterval(timeout)
        while process.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.02)
        }
        if process.isRunning {
            Self.terminateChildProcesses(parentPID: process.processIdentifier)
            kill(process.processIdentifier, SIGTERM)
            Thread.sleep(forTimeInterval: 0.15)
            if process.isRunning {
                Self.terminateChildProcesses(parentPID: process.processIdentifier, force: true)
                kill(process.processIdentifier, SIGKILL)
            }
            stderrPipe.fileHandleForReading.readabilityHandler = nil
            throw EngineError.commandFailed("操作超时（超过 \(Int(timeout)) 秒）。")
        }

        stderrPipe.fileHandleForReading.readabilityHandler = nil
        let tailErr = stderrPipe.fileHandleForReading.readDataToEndOfFile()
        if !tailErr.isEmpty {
            errBox.appendStderrChunk(tailErr, onProgress: onProgress)
        }
        _ = readGroup.wait(timeout: .now() + 1.0)

        let stdoutData = outBox.getData()
        let stderrText = errBox.finalizeStderr()

        guard process.terminationStatus == 0 else {
            let fallback = String(decoding: stdoutData, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            throw EngineError.commandFailed(stderrText.isEmpty ? fallback : stderrText)
        }
        return stdoutData
    }

    // MARK: - macOS 登录 Shell 环境与常见 CLI 路径补齐

    private func resolvedEnvironment() async -> [String: String] {
        if let cached = envCache.get() {
            return cached
        }
        let built = await Task.detached(priority: .userInitiated) { () -> [String: String] in
            var env = ProcessInfo.processInfo.environment
            let home = FileManager.default.homeDirectoryForCurrentUser
            let shell = env["SHELL"] ?? "/bin/zsh"
            let loginPath = Self.readLoginShellPath(shell: shell, baseEnv: env) ?? (env["PATH"] ?? "")

            var extraDirs: [String] = [
                "/opt/homebrew/bin",
                "/opt/homebrew/sbin",
                "/usr/local/bin",
                home.appendingPathComponent(".local/bin").path,
                home.appendingPathComponent(".claude/local").path,
                home.appendingPathComponent(".cargo/bin").path,
                home.appendingPathComponent(".bun/bin").path,
                home.appendingPathComponent(".volta/bin").path,
                home.appendingPathComponent("Library/pnpm").path,
                home.appendingPathComponent(".local/share/mise/shims").path,
                home.appendingPathComponent(".asdf/shims").path,
                home.appendingPathComponent(".pyenv/shims").path,
                home.appendingPathComponent(".kimi-code/bin").path,
                home.appendingPathComponent(".antigravity/bin").path,
                "/Applications/Antigravity.app/Contents/Resources/app/bin",
                "/Applications/Cursor.app/Contents/Resources/app/bin"
            ]
            if let nvmBin = Self.latestNVMBin(home: home) {
                extraDirs.insert(nvmBin, at: 0)
            }

            var seen = Set<String>()
            var parts: [String] = []
            for raw in loginPath.split(separator: ":").map(String.init) + extraDirs {
                let clean = raw.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !clean.isEmpty, seen.insert(clean).inserted else { continue }
                parts.append(clean)
            }
            env["PATH"] = parts.joined(separator: ":")
            return env
        }.value
        envCache.set(built)
        return built
    }

    private static func readLoginShellPath(shell: String, baseEnv: [String: String]) -> String? {
        guard FileManager.default.isExecutableFile(atPath: shell) else { return nil }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: shell)
        proc.arguments = ["-l", "-c", #"printf 'AICROSS_PATH=%s\n' "$PATH""#]
        proc.environment = baseEnv
        proc.standardInput = FileHandle.nullDevice
        let out = Pipe()
        proc.standardOutput = out
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
        } catch {
            return nil
        }
        let deadline = Date().addingTimeInterval(4.5)
        while proc.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.02)
        }
        if proc.isRunning {
            kill(proc.processIdentifier, SIGKILL)
            return nil
        }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        let text = String(decoding: data, as: UTF8.self)
        for line in text.components(separatedBy: .newlines).reversed() {
            if let r = line.range(of: "AICROSS_PATH=") {
                let val = String(line[r.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
                if !val.isEmpty { return val }
            }
        }
        return nil
    }

    private static func latestNVMBin(home: URL) -> String? {
        let root = home.appendingPathComponent(".nvm/versions/node", isDirectory: true)
        guard let dirs = try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil) else {
            return nil
        }
        let sorted = dirs.sorted { a, b in
            a.lastPathComponent.compare(b.lastPathComponent, options: .numeric) == .orderedAscending
        }
        for d in sorted.reversed() {
            let bin = d.appendingPathComponent("bin")
            if FileManager.default.fileExists(atPath: bin.path) {
                return bin.path
            }
        }
        return nil
    }

    static func stripQuarantine(at path: String) {
        guard FileManager.default.isExecutableFile(atPath: "/usr/bin/xattr") else { return }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/xattr")
        proc.arguments = ["-dr", "com.apple.quarantine", path]
        proc.standardInput = FileHandle.nullDevice
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        try? proc.run()
        proc.waitUntilExit()
    }

    private static func terminateChildProcesses(parentPID: pid_t, force: Bool = false) {
        guard FileManager.default.isExecutableFile(atPath: "/usr/bin/pkill") else { return }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        proc.arguments = [force ? "-KILL" : "-TERM", "-P", "\(parentPID)"]
        proc.standardInput = FileHandle.nullDevice
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        try? proc.run()
        proc.waitUntilExit()
    }
}
