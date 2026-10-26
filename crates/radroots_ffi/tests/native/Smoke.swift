import Foundation

enum SmokeFailure: Swift.Error {
    case failed(String)
}
func trace(_ message: String) {
    FileHandle.standardOutput.write(Data((message + "\n").utf8))
}
func check(_ value: Bool, _ message: String) throws {
    if !value { throw SmokeFailure.failed(message) }
}
func request(_ host: Host, _ id: String) throws -> CommandRequest {
    CommandRequest(operationId: String(repeating: id, count: 32),
        firstIssueNotAfter: String(UInt64(try host.unixSeconds())! + 120), correlationId: "Swift actual calls")
}
func listing(_ host: Host, _ id: String) throws -> ListingInput {
    let option = String(repeating: "02", count: 32)
    let profile: [String: Any] = [
        "profile": "radroots.listing", "version": 1, "state": "active",
        "product": ["name": "Carrots", "description": "Fresh carrots", "producer": NSNull(), "origin_summary": NSNull()],
        "default_option": option,
        "options": [["id": option, "label": "Bunch", "unit": ["type": "package", "label": "bunch", "contents": ["type": "unspecified"]],
            "increment": "1", "minimum": "1", "price": ["type": "quote_required"], "supply": ["type": "available", "amount": "10"], "lead_seconds": "0"]],
        "fulfillment": ["modes": ["pickup"], "area": "Example", "lead_seconds": "0"], "authority": NSNull(), "media": []
    ]
    let input: [String: Any] = ["id": String(repeating: id, count: 16), "title": "  Swift carrots 🥕  ",
        "summary": "Quote required", "description_markdown": "Exact native terms", "published_at": try host.unixSeconds(), "profile": profile]
    return try ListingInput.parse(input: JSONSerialization.data(withJSONObject: input))
}
func close(_ host: Host, _ session: Session) async throws {
    let callbacks = UInt64(try host.measurements().closedCallbacks)!
    try await session.client.close()
    try check(session.client.state() == .closed, "actual closed state")
    try check(UInt64(try host.measurements().closedCallbacks)! == callbacks + 1, "actual provider closure")
    try await host.cleanup()
}

func publicFlow() async throws {
    let host = try await Host.open(seller: true, unknownSigning: false, pauseAfterSigning: false)
    let first = host.session()
    try check(first.client.state() == .ready, "host does not auto-start")
    _ = try await first.client.start(mode: .online)
    host.setAccepting(accepting: false)
    var context = try request(host, "11")
    context.firstIssueNotAfter = String(UInt64(try host.unixSeconds())! + 10)
    let input = try listing(host, "11")
    let result = try await first.client.publishListing(workspace: first.workspace, request: context, input: input)
    try check(result.kind == .listing && result.admission == .confirmed, "real public admission")
    try check(result.businessInstallation == .installed && result.issuance == .obtained, "independent install/issuance")
    guard case let .listing(value)? = result.businessResult else { throw SmokeFailure.failed("typed listing result") }
    try check(value.title() == "  Swift carrots 🥕  ", "exact Unicode/whitespace")
    try check(result.messageId == nil && result.artifacts.allSatisfy { $0.recipient == nil }, "public optional identity")
    try check(result.deliveryObservations.count == 1 && result.deliveryObservations[0].outcome == .rejected, "actual rejection")
    let before = try host.measurements()
    try check(before.signingCalls == "1" && before.legacySigningCalls == "0" && before.relayReads == "0", "actual public effects")
    let event = try JSONSerialization.jsonObject(with: Data(before.originalSignedEvents[0].utf8)) as! [String: Any]
    try check(event["id"] as? String == value.revision(), "actual wire revision")
    let replay = try await first.client.publishListing(workspace: first.workspace, request: context, input: input)
    try check(replay.operationCheckpoint == result.operationCheckpoint, "replay retains facts")
    try check(try host.measurements().originalSignedEvents == before.originalSignedEvents, "replay has no network effect")
    try await first.client.close()
    let next = try await host.reopen()
    _ = try await next.client.start(mode: .online)
    let queried = try await next.client.operation(workspace: next.workspace, operationId: result.operationId, expectedDigest: result.commandDigest)
    try check(queried?.operationCheckpoint == result.operationCheckpoint, "scoped reopened query")
    host.setAccepting(accepting: true)
    while UInt64(try host.unixSeconds())! <= UInt64(result.firstIssueNotAfter)! {
        try await Task.sleep(nanoseconds: 100_000_000)
    }
    let resumed = try await next.client.resume(workspace: next.workspace, operationId: result.operationId, expectedDigest: result.commandDigest)
    try check(resumed.deliveryAttempts.count == 2, "fresh retained retry count=\(resumed.deliveryAttempts.count), issuance=\(resumed.issuance), recovery=\(resumed.recovery), error=\(String(describing: resumed.error))")
    try check(Set(resumed.deliveryAttempts.map { $0.targetOrdinals }) == Set([Data([1]), Data([2])]), "both original and fresh ordinals are retained independently of identity order")
    try check(resumed.deliveryAttempts[0].originalBytesDigest == resumed.deliveryAttempts[1].originalBytesDigest, "original byte commitment")
    try check(resumed.deliveryAttempts[0].publicationPolicyDigest == resumed.deliveryAttempts[1].publicationPolicyDigest, "original policy")
    try check(resumed.deliveryObservations.contains { $0.outcome == .accepted }, "actual acceptance")
    let after = try host.measurements()
    try check(after.signingCalls == "1" && after.originalSignedEvents == [before.originalSignedEvents[0], before.originalSignedEvents[0]], "same actual bytes, no resign")
    try await close(host, next)
}

func privateFlowAndErrors() async throws {
    let host = try await Host.open(seller: false, unknownSigning: false, pauseAfterSigning: false)
    let session = host.session(); _ = try await session.client.start(mode: .online)
    let peer = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
    let text = "\u{feff}  Cafe\u{301} / Café Swift inquiry 🥕\nExact text\0  "
    let input = try InquiryInput(threadId: String(repeating: "22", count: 32), peer: peer, source: nil, text: text)
    let result = try await session.client.inquire(workspace: session.workspace, request: request(host, "12"), input: input)
    guard case let .chat(value)? = result.businessResult else { throw SmokeFailure.failed("typed chat") }
    try check(result.kind == .chat && value.peer() == peer && value.text().utf8.elementsEqual(text.utf8), "actual private UTF8 data without normalization")
    try check(result.artifacts.count == 2 && result.artifacts.allSatisfy { $0.recipient != nil }, "original private subjects")
    let before = try host.measurements(); try check(before.signingCalls == "2" && before.relayReads != "0", "private route/signing effects")
    do {
        _ = try InquiryInput(threadId: String(repeating: "22", count: 32), peer: peer.uppercased(), source: nil, text: "private input must not enter errors")
        throw SmokeFailure.failed("uppercase key accepted")
    } catch Error.InvalidInput(let field) { try check(field == "peer", "safe input cause") }
    do { _ = try ListingInput.parse(input: Data("{}".utf8)); throw SmokeFailure.failed("invalid listing accepted") }
    catch Error.InvalidInput(let field) { try check(field == "listing_input", "shared input parser") }
    do { _ = try ListingInput.parse(input: Data("{\"profile\":{\"version\":2}}".utf8)); throw SmokeFailure.failed("unsupported profile accepted") }
    catch Error.CommandInput(let field, let kind) { try check(field == "listing_input" && kind == .unsupportedProfile, "typed unsupported profile") }
    do { _ = try ListingInput.parse(input: Data(repeating: 32, count: 65_537)); throw SmokeFailure.failed("oversize input accepted") }
    catch Error.CommandInput(let field, let kind) { try check(field == "listing_input" && kind == .limitExceeded, "typed shared bound") }
    let otherHost = try await Host.open(seller: false, unknownSigning: false, pauseAfterSigning: false)
    let other = otherHost.session(); _ = try await other.client.start(mode: .online)
    do {
        _ = try await session.client.inquire(workspace: other.workspace, request: request(host, "13"), input: input)
        throw SmokeFailure.failed("foreign scope accepted")
    } catch Error.Operation(_, _, let operation, let admission, let rejected) {
        try check(rejected == nil, "foreign admission has no signer diagnostic")
        try check(operation == String(repeating: "13", count: 32) && admission == .notAdmitted, "retained safe error context")
    }
    try check(try host.measurements().signingCalls == before.signingCalls, "refusal has no signature")
    try check(try otherHost.measurements().signingCalls == "0", "foreign host unused")
    session.workspace.close()
    do {
        _ = try await session.client.inquire(workspace: session.workspace, request: request(host, "16"), input: input)
        throw SmokeFailure.failed("closed attachment accepted")
    } catch Error.Operation(_, _, _, let admission, let rejected) {
        try check(rejected == nil, "closed admission has no signer diagnostic")
        try check(admission == .notAdmitted, "actual attachment close releases authority")
    }
    try check(try host.measurements().signingCalls == before.signingCalls, "closed scope has no signature")
    try await close(otherHost, other); try await close(host, session)
}

func unknownFlow() async throws {
    let host = try await Host.open(seller: true, unknownSigning: true, pauseAfterSigning: false)
    let session = host.session(); _ = try await session.client.start(mode: .online)
    let result = try await session.client.publishListing(workspace: session.workspace, request: request(host, "14"), input: listing(host, "14"))
    try check(result.issuance == .unknown && result.businessResult == nil, "unknown result remains explicit")
    try await session.client.close(); host.setUnknownSigning(unknown: false)
    let next = try await host.reopen(); _ = try await next.client.start(mode: .online)
    let resumed = try await next.client.resume(workspace: next.workspace, operationId: result.operationId, expectedDigest: result.commandDigest)
    try check(resumed.issuance == .unknown && resumed.businessResult == nil, "reopen cannot turn uncertainty into permission")
    let measured = try host.measurements(); try check(measured.signingCalls == "1" && measured.originalSignedEvents.isEmpty, "no repeated invocation")
    try await close(host, next)
}

func cancelledObservers() async throws {
    let host = try await Host.open(seller: true, unknownSigning: false, pauseAfterSigning: true)
    let session = host.session(); _ = try await session.client.start(mode: .online)
    let context = try request(host, "15"); let input = try listing(host, "15")
    let observer = Task { try await session.client.publishListing(workspace: session.workspace, request: context, input: input) }
    try await host.waitUntilSignerPaused()
    for _ in 0..<2 {
        let safe = await withTaskGroup(of: Bool.self) { group in
            for index in 0..<16 {
                group.addTask {
                    let query = Task { try await session.client.operation(workspace: session.workspace, operationId: context.operationId, expectedDigest: nil) }
                    if index % 2 == 0 { await Task.yield() }
                    query.cancel()
                    do { _ = try await query.value; return true }
                    catch is CancellationError { return true }
                    catch { return false }
                }
            }
            var accepted = true
            for await outcome in group { accepted = accepted && outcome }
            return accepted
        }
        try check(safe, "cancel/completion race preserves safe future lifetime")
    }
    observer.cancel(); _ = await observer.result
    let closing = Task { try await session.client.close() }
    for _ in 0..<200 where session.client.state() != .closing { try await Task.sleep(nanoseconds: 5_000_000) }
    try check(session.client.state() == .closing, "close owns pending produced signature")
    closing.cancel(); _ = await closing.result; host.releaseSigner(); try await session.client.close()
    let next = try await host.reopen(); _ = try await next.client.start(mode: .online)
    let recovered = try await next.client.resume(workspace: next.workspace, operationId: context.operationId, expectedDigest: nil)
    try check(recovered.issuance == .obtained && recovered.businessInstallation == .installed, "actual obtained result recovered")
    try check(try host.measurements().signingCalls == "1", "observer loss never resigns")
    try await close(host, next)
}


func rejectedSignerFlow() async throws {
    let cases: [(FixtureRejection, SignerRejectionReason, String?, String?, String?)] = [
        (.malformed, .invalidArtifact, String("synthetic private rejection canary".utf8.count), String("synthetic private rejection canary".utf8.count), "ba883ea08dff4c078672a055dd2be042d9debb5b22c66527e4499c068011879d"),
        (.oversized, .tooLarge, "65536", "65537", "d2d5af5c0316f5c2b119635386239edc87abce035f851c3f6ee127e6c3cbf435"),
        (.foreign, .bindingMismatch, nil, nil, nil)
    ]
    for (mode, reason, covered, total, digest) in cases {
        let host = try await Host.open(seller: true, unknownSigning: false, pauseAfterSigning: false)
        host.setRejectedSigning(mode: mode)
        let session = host.session(); _ = try await session.client.start(mode: .online)
        let context = try request(host, "18")
        do {
            _ = try await session.client.publishListing(workspace: session.workspace, request: context, input: listing(host, "18"))
            throw SmokeFailure.failed("rejected signer accepted")
        } catch Error.Operation(let code, let retry, let operation, let admission, let diagnostic) {
            try check(code == "signer_tampering" && retry == "manual_reconciliation", "original rejection code/retry")
            try check(operation == context.operationId && admission == .confirmed, "original confirmed command")
            guard let diagnostic else { throw SmokeFailure.failed("lost bounded signer metadata") }
            try check(diagnostic.reason == reason, "fixed rejection reason")
            if let fingerprint = diagnostic.fingerprint {
                try check(fingerprint.coveredBytes == covered && fingerprint.totalBytes == total, "exact prefix coverage")
                try check(fingerprint.sha256.count == 32, "bounded SHA-256")
                let actual = fingerprint.sha256.map { String(format: "%02x", $0) }.joined()
                try check(actual == digest, "actual original prefix hash")
            } else {
                try check(digest == nil, "no fabricated/missing fingerprint")
            }
        }
        let facts = try await session.client.operation(workspace: session.workspace, operationId: context.operationId, expectedDigest: nil)
        try check(facts?.businessInstallation == .notInstalled, "rejection is no business success")
        let measured = try host.measurements()
        try check(measured.signingCalls == "1" && measured.legacySigningCalls == "0" && measured.originalSignedEvents.isEmpty, "no signature or legacy fallback")
        try await close(host, session)
    }
}


@main enum NativeSmoke {
    static func main() async throws {
        if CommandLine.arguments.contains("--plain-panic") { _ = fixturePanicProbe(); throw SmokeFailure.failed("plain panic returned") }
        var resultPanicCaught = false
        do { _ = try fixtureResultPanicProbe() } catch { resultPanicCaught = true }
        try check(resultPanicCaught, "actual Result panic boundary")
        let host = try await Host.open(seller: false, unknownSigning: false, pauseAfterSigning: false)
        let session = host.session()
        let alreadyCancelled = Task {
            do { try await Task.sleep(nanoseconds: 5_000_000_000) } catch {}
            return try await session.client.start(mode: .online)
        }
        alreadyCancelled.cancel()
        do { _ = try await alreadyCancelled.value; throw SmokeFailure.failed("already cancelled call started") }
        catch is CancellationError {}
        try check(session.client.state() == .ready, "already cancelled binding call has no host effect")
        try await close(host, session)
        trace("swift already-cancelled call passed")
        try await publicFlow(); trace("swift public workflow passed")
        try await privateFlowAndErrors(); trace("swift private workflow passed")
        try await unknownFlow(); trace("swift unknown workflow passed")
        try await cancelledObservers(); trace("swift cancellation workflow passed")
        try await rejectedSignerFlow(); trace("swift bounded rejection workflow passed")
        print("{\"language\":\"swift\",\"state\":\"passed\",\"actual_workflows\":5,\"result_panic_caught\":true}")
    }
}
