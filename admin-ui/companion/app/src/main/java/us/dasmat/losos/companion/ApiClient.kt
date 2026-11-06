package us.dasmat.losos.companion

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToLong

/*
 * The box's JSON API, driven the way the admin SPA drives it
 * (admin-ui/app/src/lib/api.ts): every document answers at <base>/api/… and
 * every route except /api/health wants `Authorization: Bearer <token>` — the
 * 64-hex key the box minted at first boot. A 401 is the session ending: the
 * token goes and the app asks again, like the SPA's eviction (api.ts, the
 * dropToken on 401 that the app shell subscribes to).
 */

/** A non-2xx from lososd; the message carries the box's own error word. */
class ApiError(val status: Int, message: String) : IOException(message)

/** The API of one box, with the owner's token in every call it makes. */
class BoxApi(
    address: String,
    private val token: String,
    /** A rejected token's fate, wired by the caller: flip to the connect screen. */
    private val onUnauthorized: () -> Unit = {},
) {
    private val base =
        if (address.startsWith("http")) address.trimEnd('/') else "https://${address.trimEnd('/')}"

    /** GET a document; /api/health is fetched anonymous, like the SPA's health call. */
    suspend fun get(path: String, anonymous: Boolean = false): JSONObject =
        call(path, "GET", null, anonymous)

    /** POST a bodyless action. */
    suspend fun post(path: String): JSONObject = call(path, "POST", null, false)

    /** POST JSON; app install/remove and other actions carry a body. */
    suspend fun post(path: String, body: JSONObject): JSONObject =
        call(path, "POST", body.toString(), false)

    private suspend fun call(
        path: String,
        method: String,
        body: String?,
        anonymous: Boolean,
    ): JSONObject = withContext(Dispatchers.IO) {
        if (base.isBlank()) throw ApiError(0, "No address set")
        val conn = URL(base + path).openConnection() as HttpURLConnection
        try {
            BoxTrust.install(conn)
            conn.connectTimeout = 4000
            conn.readTimeout = 20000
            conn.instanceFollowRedirects = false
            conn.requestMethod = method
            // The token rides only real API calls; reachability probes never see it.
            if (!anonymous) conn.setRequestProperty("Authorization", "Bearer $token")
            if (body != null) {
                conn.setRequestProperty("Content-Type", "application/json")
                conn.doOutput = true
                conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
            }
            when (val code = conn.responseCode) {
                401 -> {
                    onUnauthorized()
                    throw ApiError(401, "The box wants its token again")
                }
                !in 200..299 -> throw ApiError(code, errorText(conn, code))
            }
            val text = conn.inputStream?.use { it.readBytes().decodeToString() }.orEmpty()
            if (text.isNotEmpty()) JSONObject(text) else JSONObject()
        } finally {
            conn.disconnect()
        }
    }

    /* The box's error body is {"error": "..."}; anything else stays the HTTP word. */
    private fun errorText(conn: HttpURLConnection, code: Int): String = try {
        conn.errorStream?.use { it.readBytes().decodeToString() }
            ?.takeIf { it.isNotEmpty() }
            ?.let { JSONObject(it).optString("error") }
            ?.takeIf { it.isNotEmpty() }
            ?: "HTTP $code"
    } catch (_: Exception) {
        "HTTP $code"
    }
}

internal fun encodeQuery(value: String): String =
    URLEncoder.encode(value, Charsets.UTF_8.name())

/* Field reads in the pages' spirit (admin-ui/app/src/lib/api.ts): a field
 * that is missing reads as absent, never as a failed card. Names follow the
 * wire, camelCase, so every route's types line up with the SPA's. */

internal fun JSONObject.maybeString(name: String): String? = when (val v = opt(name)) {
    null, JSONObject.NULL -> null
    is String -> v.ifBlank { null }
    else -> null
}

internal fun JSONObject.maybeNumber(name: String): Long? = when (val v = opt(name)) {
    is Number -> v.toLong()
    else -> null
}

internal fun JSONObject.maybeDouble(name: String): Double? = when (val v = opt(name)) {
    is Number -> v.toDouble()
    else -> null
}

internal fun JSONObject.maybeBool(name: String, dflt: Boolean = false): Boolean =
    opt(name) as? Boolean ?: dflt

internal fun JSONObject.maybeObject(name: String): JSONObject? = opt(name) as? JSONObject

internal fun JSONObject.itemsOf(name: String): List<JSONObject> = when (val v = opt(name)) {
    is JSONArray -> (0 until v.length()).mapNotNull { v.optJSONObject(it) }
    else -> emptyList()
}

/* The shapes a pane shows, formatted the terminal client's way (tui i18n's
 * format_bytes / format_iso / format_unix). */

internal fun formatBytes(bytes: Double?): String {
    if (bytes == null || bytes.isNaN() || bytes < 0.0) return "–"
    val gib = bytes / 1073741824.0
    return when {
        gib >= 1024.0 -> "%.1f TB".format(gib / 1024.0)
        gib >= 10.0 -> "${gib.roundToLong()} GB"
        gib >= 1.0 -> "%.1f GB".format(gib)
        else -> "${(gib * 1024.0).roundToLong()} MB"
    }
}

/** ISO 8601 down to the minute: 2026-01-02T10:33:22Z → 2026-01-02 10:33. */
internal fun formatStamp(iso: String?): String =
    iso?.take(16)?.replace("T", " ") ?: "–"

internal fun formatUnix(seconds: Long?): String {
    val at = seconds ?: return "–"
    return SimpleDateFormat("yyyy-MM-dd HH:mm", Locale.getDefault()).format(Date(at * 1000L))
}

internal fun shortSha(sha: String?): String = sha?.take(7) ?: "–"

/* The compute window, in words (admin-ui/app/src/screens/settings/window.ts). */

internal fun minutesOf(hhmm: String): Int? {
    if (hhmm.length != 5 || hhmm[2] != ':') return null
    val h = hhmm.take(2).toIntOrNull() ?: return null
    val m = hhmm.takeLast(2).toIntOrNull() ?: return null
    if (h !in 0..23 || m !in 0..59) return null
    return h * 60 + m
}

internal fun describeWindow(start: String, end: String): String {
    val s = minutesOf(start) ?: return "Not set"
    val e = minutesOf(end) ?: return "Not set"
    if (s == e) return "Set, to nothing"
    val total = if (s < e) e - s else (1440 - s) + e
    val length = if (total % 60 == 0) "${total / 60} h" else "$total min"
    val shape = if (s > e) ", wrapping midnight" else ""
    return "$start–$end, $length a day$shape"
}
