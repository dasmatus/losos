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

/* Trust for the box's self-signed certificate, shared by the reachability
 * probe and the API client (ApiClient.kt): every box serves one on the LAN
 * until the owner enrols a CA. */
internal object BoxTrust {
    private val permissiveTm = object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {}
        override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
    }

    private val context: SSLContext by lazy {
        SSLContext.getInstance("TLS").apply {
            init(null, arrayOf<TrustManager>(permissiveTm), SecureRandom())
        }
    }

    fun install(conn: HttpURLConnection) {
        (conn as? HttpsURLConnection)?.let {
            it.sslSocketFactory = context.socketFactory
            it.setHostnameVerifier { _, _ -> true }
        }
    }
}

object BoxProbe {
    /**
     * GET the box's admin homepage. Reachability is not identity, so the
     * round trip carries no token at all — the probe stays anonymous; the
     * calls that do carry it live in ApiClient.kt. */
    suspend fun probe(config: BoxConfig): ProbeResult = withContext(Dispatchers.IO) {
        if (config.host.isBlank()) return@withContext ProbeResult.Unreachable("No address set")
        try {
            val conn = URL(config.base).openConnection() as HttpURLConnection
            conn.connectTimeout = 4000
            conn.readTimeout = 4000
            conn.instanceFollowRedirects = false
            conn.requestMethod = "GET"
            BoxTrust.install(conn)
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
