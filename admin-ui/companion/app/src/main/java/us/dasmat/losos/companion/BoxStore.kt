package us.dasmat.losos.companion

import android.content.Context
import java.net.HttpURLConnection
import java.net.URL
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager
import java.security.SecureRandom
import java.security.cert.X509Certificate
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Where the box lives, and the access token that links us to it. */
data class BoxConfig(
    val host: String = "",
    val token: String = "",
    val demo: Boolean = true,
    val themeMode: ThemeMode = ThemeMode.System,
) {
    val base: String get() = if (host.startsWith("http")) host.trimEnd('/') else "https://$host"
}

/** A probe round trip to the box's admin pages. */
sealed interface ProbeResult {
    data class Reachable(val ms: Long) : ProbeResult
    data class Unreachable(val reason: String) : ProbeResult
    data object Probing : ProbeResult
}

object BoxStore {
    private const val PREFS = "losos-companion"

    fun load(context: Context): BoxConfig {
        val p = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        return BoxConfig(
            host = p.getString("host", "") ?: "",
            token = p.getString("token", "") ?: "",
            demo = p.getBoolean("demo", true),
            themeMode = ThemeMode.entries.toTypedArray()
                .firstOrNull { it.name == p.getString("theme", "System") } ?: ThemeMode.System,
        )
    }

    fun save(context: Context, config: BoxConfig) {
        p(context).edit()
            .putString("host", config.host)
            .putString("token", config.token)
            .putBoolean("demo", config.demo)
            .putString("theme", config.themeMode.name)
            .apply()
    }

    private fun p(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}

object BoxProbe {
    private val permissiveTm = object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
    }

    /**
     * GET the box's admin homepage. Every box serves a self-signed
     * certificate on the LAN until the owner enrols a CA, so the probe
     * trusts whatever certificate answers and reports the round trip.
     */
    suspend fun probe(config: BoxConfig): ProbeResult = withContext(Dispatchers.IO) {
        if (config.host.isBlank()) return@withContext ProbeResult.Unreachable("No address set")
        try {
            val trustCtx = SSLContext.getInstance("TLS")
            trustCtx.init(null, arrayOf<TrustManager>(permissiveTm), SecureRandom())
            val conn = URL(config.base).openConnection()
                as HttpsURLConnection
            conn.connectTimeout = 4000
            conn.readTimeout = 4000
            conn.instanceFollowRedirects = false
            conn.requestMethod = "GET"
            if (conn is HttpsURLConnection) {
                conn.sslSocketFactory = trustCtx.socketFactory
                conn.setHostnameVerifier { _, _ -> true }
            }
            (conn as HttpURLConnection).setRequestProperty("Authorization", "Bearer ${config.token}")
            val started = System.currentTimeMillis()
            conn.responseCode
            val ms = System.currentTimeMillis() - started
            conn.disconnect()
            ProbeResult.Reachable(ms)
        } catch (e: Exception) {
            ProbeResult.Unreachable(e.message ?: e.javaClass.simpleName)
        }
    }

    /** Demo data stand-ins for the admin pages' cards, until a real box is linked. */
    val demoStorage = StorageSnapshot(
        usedGb = 184, totalGb = 512, label = "1 TB, encrypted /persist",
    )
}

data class StorageSnapshot(val usedGb: Int, val totalGb: Int, val label: String) {
    val fraction: Float get() = usedGb.toFloat() / totalGb
    val phrase: String get() = "${usedGb} GB used of ${totalGb} GB"
}
