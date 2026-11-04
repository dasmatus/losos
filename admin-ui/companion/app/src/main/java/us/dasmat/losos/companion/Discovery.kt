package us.dasmat.losos.companion

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import kotlin.coroutines.resume
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/*
 * Boxes on the LAN, through DNS-SD: every box announces `_losos._tcp` on
 * the front vhost's port with its release tag in TXT (modules/configuration.nix,
 * modules/mdns.nix) — the one browse type LosOS Desktop lists boxes by, so
 * the companion browses the same record instead of knocking on workstations.
 */

const val LOSOS_SERVICE_TYPE = "_losos._tcp."

/** One answer to the browse. `name` is the box's host name — its
 *  `<name>.local` on this LAN — and `port` the front vhost's as announced. */
data class FoundBox(
    val name: String,
    val ip: String?,
    val port: Int,
    val version: String? = null,
) {
    /** How to reach it: the `.local` name the handbook's way in, which every
     *  mDNS-speaking phone resolves; `ip` is the fallback when it will not. */
    val address: String get() = "$name.local"
}

/** Browse `_losos._tcp` for `timeoutMs`, resolve what answered, stop. The
 *  name needs no resolution (it is the host name), but resolution carries
 *  the announced port and the TXT release tag, and an IP for networks
 *  whose phone-side `.local` lookup is broken. */
suspend fun discoverBoxes(context: Context, timeoutMs: Long = 3_000): List<FoundBox> =
    withContext(Dispatchers.Main) {
        val nsd = context.getSystemService(NsdManager::class.java)
            ?: return@withContext emptyList()
        val found = LinkedHashSet<NsdServiceInfo>()
        val settled = CompletableDeferred<Unit>()

        val listener = object : NsdManager.DiscoveryListener {
            override fun onServiceFound(info: NsdServiceInfo) {
                synchronized(found) { found += info }
            }

            override fun onServiceLost(info: NsdServiceInfo) {
                synchronized(found) { found.removeAll { it.serviceName == info.serviceName } }
            }

            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {
                settled.complete(Unit)
            }

            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                settled.complete(Unit)
            }

            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
                settled.complete(Unit)
            }
        }

        nsd.discoverServices(LOSOS_SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
        // Ordinary path: the timeout ends the browse window.
        withTimeoutOrNull(timeoutMs) { settled.await() }
        runCatching { nsd.stopServiceDiscovery(listener) }

        val infos = synchronized(found) { found.toList() }
        // Historical Android resolves one service at a time; take them in turn.
        infos.mapNotNull { info -> resolveBox(nsd, info) }
    }

private suspend fun resolveBox(nsd: NsdManager, info: NsdServiceInfo): FoundBox? =
    withTimeoutOrNull(2_000) {
        suspendCancellableCoroutine { cont ->
            val listener = object : NsdManager.ResolveListener {
                override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {
                    cont.resume(null)
                }

                override fun onServiceResolved(info: NsdServiceInfo) {
                    cont.resume(
                        FoundBox(
                            name = info.serviceName,
                            ip = info.host?.hostAddress,
                            port = info.port,
                            version = info.attributes["version"]
                                ?.toString(Charsets.UTF_8),
                        ),
                    )
                }
            }
            runCatching { nsd.resolveService(info, listener) }
                .onFailure { cont.resume(null) }
        }
    }
