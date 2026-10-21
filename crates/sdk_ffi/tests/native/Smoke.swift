import Foundation

enum SmokeFailure: Swift.Error { case failed(String) }
func check(_ value: Bool, _ message: String) throws {
    if !value { throw SmokeFailure.failed(message) }
}
@main
struct Smoke {
    static func main() async throws {
        do {
            _ = try parseTradeEvidenceManifest(canonicalBytes: Data([0]))
            throw SmokeFailure.failed("invalid manifest accepted")
        } catch let failure as Error {
            switch failure {
            case let .Sdk(code, message, retryable):
                try check(code == "invalid_evidence_manifest" && message == "evidence manifest is invalid" && !retryable, "all original safe native error fields")
            }
        }
        let client = try MobileClient.memory()
        try check(!client.isClosed() && !client.capabilities().isEmpty, "actual retained memory client")
        try await client.close()
        try check(client.isClosed(), "actual asynchronous native close")
        try await client.close()
        let alreadyCancelled = Task { () throws -> Void in
            withUnsafeCurrentTask { $0?.cancel() }
            try await client.close()
        }
        do {
            try await alreadyCancelled.value
            throw SmokeFailure.failed("cancelled observer succeeded")
        } catch is CancellationError { }
        try await client.close()
        print("retained Swift native fields, lifecycle and observer cancellation passed")
    }
}
