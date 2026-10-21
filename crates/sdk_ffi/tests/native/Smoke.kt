import kotlinx.coroutines.runBlocking
import uniffi.radroots_sdk.MobileClient
import uniffi.radroots_sdk.Exception as SdkFailure
import uniffi.radroots_sdk.parseTradeEvidenceManifest

fun main() = runBlocking {
    try {
        parseTradeEvidenceManifest(byteArrayOf(0))
        error("invalid manifest accepted")
    } catch (failure: SdkFailure.Sdk) {
        check(failure.code == "invalid_evidence_manifest")
        check(failure.message == "evidence manifest is invalid")
        check(!failure.retryable)
    }
    val client = MobileClient.memory()
    check(!client.isClosed() && client.capabilities().isNotEmpty())
    client.close()
    check(client.isClosed())
    client.close()
    client.destroy()
    println("retained Kotlin native fields and actual asynchronous lifecycle passed")
}
