import kotlinx.coroutines.*
import uniffi.radroots_ffi.*
import uniffi.radroots_ffi.Exception as RadrootsException
import uniffi.radroots_native_binding_fixture.*

private fun request(host: Host, id: String, seconds: Long = 120) = CommandRequest(
    id.repeat(32), (host.unixSeconds().toLong() + seconds).toString(), "Kotlin actual calls")

private fun listing(host: Host, id: String): ListingInput {
    val option = "02".repeat(32)
    val json = """{"id":"${id.repeat(16)}","title":"  Kotlin carrots 🥕  ","summary":"Quote required","description_markdown":"Exact native terms","published_at":"${host.unixSeconds()}","profile":{"profile":"radroots.listing","version":1,"state":"active","product":{"name":"Carrots","description":"Fresh carrots","producer":null,"origin_summary":null},"default_option":"$option","options":[{"id":"$option","label":"Bunch","unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},"increment":"1","minimum":"1","price":{"type":"quote_required"},"supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],"fulfillment":{"modes":["pickup"],"area":"Example","lead_seconds":"0"},"authority":null,"media":[]}}"""
    return ListingInput.parse(json.toByteArray(Charsets.UTF_8))
}

private suspend fun close(host: Host, session: Session) {
    val callbacks = host.measurements().closedCallbacks.toLong()
    session.client.close()
    check(session.client.state() == SessionState.CLOSED)
    check(host.measurements().closedCallbacks.toLong() == callbacks + 1)
    host.cleanup()
    // Only after actual resource closure release the language's references.
    session.workspace.destroy()
    session.client.destroy()
    host.destroy()
}

private suspend fun publicFlow() {
    val host = Host.open(true, false, false)
    val first = host.session()
    check(first.client.state() == SessionState.READY)
    first.client.start(StartMode.ONLINE)
    host.setAccepting(false)
    val context = request(host, "31", 10)
    val input = listing(host, "31")
    val receipt = first.client.publishListing(first.workspace, context, input)
    check(receipt.kind == OperationKind.LISTING && receipt.admission == Admission.CONFIRMED)
    check(receipt.businessInstallation == BusinessInstallation.INSTALLED && receipt.issuance == Issuance.OBTAINED)
    val value = (receipt.businessResult as Value.Listing).listing
    check(value.title() == "  Kotlin carrots 🥕  ")
    check(receipt.messageId == null && receipt.artifacts.all { it.recipient == null })
    check(receipt.deliveryObservations.single().outcome == TargetObservation.REJECTED)
    val before = host.measurements()
    check(before.signingCalls == "1" && before.legacySigningCalls == "0" && before.relayReads == "0")
    check(before.originalSignedEvents.single().contains(value.revision()))
    val replay = first.client.publishListing(first.workspace, context, input)
    check(replay.operationCheckpoint == receipt.operationCheckpoint)
    check(host.measurements().originalSignedEvents == before.originalSignedEvents)
    first.client.close()
    val next = host.reopen()
    next.client.start(StartMode.ONLINE)
    val query = next.client.operation(next.workspace, receipt.operationId, receipt.commandDigest)
    check(query?.operationCheckpoint == receipt.operationCheckpoint)
    check(host.measurements().originalSignedEvents == before.originalSignedEvents)
    while (host.unixSeconds().toLong() <= receipt.firstIssueNotAfter.toLong()) delay(100)
    host.setAccepting(true)
    val resumed = next.client.resume(next.workspace, receipt.operationId, receipt.commandDigest)
    check(resumed.firstIssueNotAfter == receipt.firstIssueNotAfter)
    check(resumed.deliveryAttempts.size == 2)
    check(resumed.deliveryAttempts.map { it.targetOrdinals.toList() }.toSet() == setOf(listOf(1.toByte()), listOf(2.toByte())))
    check(resumed.deliveryAttempts[0].originalBytesDigest == resumed.deliveryAttempts[1].originalBytesDigest)
    check(resumed.deliveryAttempts[0].publicationPolicyDigest == resumed.deliveryAttempts[1].publicationPolicyDigest)
    check(resumed.deliveryObservations.any { it.outcome == TargetObservation.ACCEPTED })
    val after = host.measurements()
    check(after.signingCalls == "1" && after.originalSignedEvents == List(2) { before.originalSignedEvents.single() })
    close(host, next)
}

private suspend fun privateFlowAndErrors() {
    val host = Host.open(false, false, false)
    val session = host.session()
    session.client.start(StartMode.ONLINE)
    val peer = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
    val text = "\uFEFF  Cafe\u0301 / Café Kotlin inquiry 🥕\nExact text\u0000  "
    val input = InquiryInput("32".repeat(32), peer, null, text)
    val receipt = session.client.inquire(session.workspace, request(host, "32"), input)
    val message = (receipt.businessResult as Value.Chat).message
    check(receipt.kind == OperationKind.CHAT && message.text().toByteArray().contentEquals(text.toByteArray()) && message.peer() == peer)
    check(receipt.artifacts.size == 2 && receipt.artifacts.all { it.recipient != null })
    val before = host.measurements()
    check(before.signingCalls == "2" && before.relayReads != "0")
    try {
        InquiryInput("32".repeat(32), peer.uppercase(), null, "must not enter error details")
        error("uppercase key accepted")
    } catch (e: RadrootsException.InvalidInput) { check(e.field == "peer") }
    try {
        ListingInput.parse("{}".toByteArray())
        error("invalid listing accepted")
    } catch (e: RadrootsException.InvalidInput) { check(e.field == "listing_input") }
    try {
        ListingInput.parse("""{"profile":{"version":2}}""".toByteArray())
        error("unsupported profile accepted")
    } catch (e: RadrootsException.CommandInput) { check(e.kind == InputFailure.UNSUPPORTED_PROFILE) }
    try {
        ListingInput.parse(ByteArray(65_537) { 32 })
        error("oversize input accepted")
    } catch (e: RadrootsException.CommandInput) { check(e.kind == InputFailure.LIMIT_EXCEEDED) }
    val foreign = Host.open(false, false, false)
    val other = foreign.session()
    other.client.start(StartMode.ONLINE)
    val context = request(host, "33")
    try {
        session.client.inquire(other.workspace, context, input)
        error("foreign scope accepted")
    } catch (e: RadrootsException.Operation) {
        check(e.admission == Admission.NOT_ADMITTED && e.operationId == context.operationId)
    }
    check(host.measurements().signingCalls == before.signingCalls)
    check(foreign.measurements().signingCalls == "0")
    session.workspace.close()
    check(!session.workspace.uniffiIsDestroyed)
    try {
        session.client.inquire(session.workspace, request(host, "36"), input)
        error("closed attachment accepted")
    } catch (e: RadrootsException.Operation) { check(e.admission == Admission.NOT_ADMITTED) }
    check(host.measurements().signingCalls == before.signingCalls)
    close(foreign, other)
    close(host, session)
}

private suspend fun unknownFlow() {
    val host = Host.open(true, true, false)
    val session = host.session()
    session.client.start(StartMode.ONLINE)
    val receipt = session.client.publishListing(session.workspace, request(host, "34"), listing(host, "34"))
    check(receipt.issuance == Issuance.UNKNOWN && receipt.businessResult == null)
    session.client.close()
    host.setUnknownSigning(false)
    val next = host.reopen()
    next.client.start(StartMode.ONLINE)
    val resumed = next.client.resume(next.workspace, receipt.operationId, receipt.commandDigest)
    check(resumed.issuance == Issuance.UNKNOWN && resumed.businessResult == null)
    val measured = host.measurements()
    check(measured.signingCalls == "1" && measured.originalSignedEvents.isEmpty())
    close(host, next)
}

private suspend fun cancelledObservers() = coroutineScope {
    val host = Host.open(true, false, true)
    val session = host.session()
    session.client.start(StartMode.ONLINE)
    val context = request(host, "35")
    val input = listing(host, "35")
    val observer = async { session.client.publishListing(session.workspace, context, input) }
    host.waitUntilSignerPaused()
    observer.cancelAndJoin()
    val closing = async { session.client.close() }
    withTimeout(5_000) {
        while (session.client.state() != SessionState.CLOSING) delay(5)
    }
    closing.cancelAndJoin()
    host.releaseSigner()
    session.client.close()
    val next = host.reopen()
    next.client.start(StartMode.ONLINE)
    val recovered = next.client.resume(next.workspace, context.operationId, null)
    check(recovered.issuance == Issuance.OBTAINED && recovered.businessInstallation == BusinessInstallation.INSTALLED)
    check(host.measurements().signingCalls == "1")
    close(host, next)
}

fun main(args: Array<String>) = runBlocking {
    if ("--plain-panic" in args) {
        var caught = false
        try { fixturePanicProbe() } catch (_: uniffi.radroots_native_binding_fixture.InternalException) { caught = true }
        check(caught)
        println("{\"language\":\"kotlin\",\"plain_panic_caught\":true}")
        return@runBlocking
    }
    var caught = false
    try { fixtureResultPanicProbe() } catch (_: uniffi.radroots_native_binding_fixture.InternalException) { caught = true }
    check(caught)
    publicFlow()
    privateFlowAndErrors()
    unknownFlow()
    cancelledObservers()
    println("{\"language\":\"kotlin\",\"state\":\"passed\",\"actual_workflows\":4,\"result_panic_caught\":true}")
}
