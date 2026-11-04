package us.dasmat.losos.companion

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

/*
 * The box's panes as phone cards: the terminal client's panes
 * (admin-ui/tui/src/tui/panes.rs) with the reads its `needs()` lists, each
 * one over the same /api/ routes the admin SPA drives. A field that is
 * missing reads as a dash rather than failing the card, in the pages'
 * spirit. A pane keeps its demo shape too; while it is on, every card that
 * carries invented numbers wears the demo chip.
 */

/** One read of one route, fresh when the pane opens — the terminal's `r`. */
sealed interface Read {
    data object Loading : Read
    data class Failed(val reason: String) : Read
    data class Done(val data: JSONObject) : Read
}

/** The box API for `config`, with the SPA's eviction: a 401 drops the token,
 *  and MainActivity's needsConnect then shows the connect screen again. */
@Composable
internal fun boxApi(config: BoxConfig, update: (BoxConfig) -> Unit): BoxApi =
    remember(config.host, config.token) {
        BoxApi(config.host, config.token) { update(config.copy(token = "")) }
    }

@Composable
internal fun ReadRoute(
    path: String,
    config: BoxConfig,
    update: (BoxConfig) -> Unit,
    anonymous: Boolean = false,
    refresh: Int = 0,
    content: @Composable (Read) -> Unit,
) {
    val api = boxApi(config, update)
    var read by remember(path, config.host, config.token, anonymous, refresh) {
        mutableStateOf<Read>(Read.Loading)
    }
    LaunchedEffect(path, config.host, config.token, anonymous, refresh) {
        read = try {
            Read.Done(api.get(path, anonymous))
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: ApiError) {
            Read.Failed(e.message ?: "HTTP ${e.status}")
        } catch (e: Exception) {
            Read.Failed(e.message ?: e.javaClass.simpleName)
        }
    }
    content(read)
}

/** The word that keeps demo data honest, on every card while demo is on. */
@Composable
fun DemoChip(modifier: Modifier = Modifier) {
    val tokens = LocalTokens.current
    Text(
        "demo",
        style = MaterialTheme.typography.labelSmall,
        color = tokens.extra.crit,
        modifier = modifier
            .clip(RoundedCornerShape(6.dp))
            .background(tokens.extra.crit.copy(alpha = 0.12f))
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

/** A pane's title and its one line, as the settings pages name them. */
@Composable
fun PaneHeader(title: String, summary: String, demo: Boolean) {
    Row(
        Modifier.padding(vertical = 12.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.headlineMedium)
            Text(
                summary,
                style = MaterialTheme.typography.bodyMedium,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
        if (demo) DemoChip()
    }
}

/**
 * A card whose body comes from one route's read: a wait, the box's own error
 * word, or the document itself.
 */
@Composable
internal fun ReadCard(title: String, read: Read, content: @Composable (JSONObject) -> Unit) {
    val tokens = LocalTokens.current
    Card2 {
        Text(title, style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(8.dp))
        when (read) {
            Read.Loading -> Text(
                "Asking the box…",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
            is Read.Failed -> Text(
                "Could not read: ${read.reason}",
                style = MaterialTheme.typography.bodySmall,
                color = tokens.extra.crit,
            )
            is Read.Done -> content(read.data)
        }
    }
}

/** One line of a pane's list: a value at the title's right, a word under it. */
@Composable
internal fun PaneRow(
    title: String,
    detail: String = "",
    value: String = "",
    tone: Color? = null,
) {
    val tokens = LocalTokens.current
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                color = tone ?: tokens.scheme.onSurface,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f, fill = false),
            )
            if (value.isNotEmpty()) {
                Spacer(Modifier.width(10.dp))
                Text(
                    value,
                    style = MaterialTheme.typography.labelSmall,
                    color = tone ?: tokens.scheme.onSurfaceVariant,
                    fontWeight = FontWeight.SemiBold,
                )
            }
        }
        if (detail.isNotEmpty()) {
            Text(
                detail,
                style = MaterialTheme.typography.bodySmall,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
    }
}

/** An install phase, named the pages' way. */
internal fun phaseWord(phase: String?): String = when (phase) {
    "installing" -> "installing"
    "removing" -> "removing"
    "failed" -> "failed"
    else -> "running"
}

// ── Dashboard: the web UI's home screen ──────────────────────────────────

@Composable
fun OverviewPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Your box", "Demo data — nothing phones home", demo = true)
        DemoOverview()
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Your box", config.base, demo = false)
    // health is anonymous, like the SPA's own header check (api.ts getHealth).
    ReadRoute("/api/health", config, update, anonymous = true) { read -> HealthCard(read, config) }
    ReadRoute("/api/status", config, update) { read -> StatusCard(read) }
    ReadRoute("/api/storage", config, update) { read -> StorageStripCard(read) }
    ReadRoute("/api/apps", config, update) { read -> OverviewAppsCard(read) }
    ReadRoute("/api/settings", config, update) { read -> LendCard(read) }
}

/** The demo half of the Overview: the fake stand-ins, every one chipped. */
@Composable
internal fun DemoOverview() {
    Card2(demo = true) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier
                .size(10.dp)
                .clip(CircleShape)
                .background(LocalTokens.current.extra.ok)) {}
            Spacer(Modifier.width(10.dp))
            Column {
                Text("Demo mode", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Link it from the connect screen to read a real box",
                    style = MaterialTheme.typography.bodySmall,
                    color = LocalTokens.current.scheme.onSurfaceVariant,
                )
            }
        }
    }
    val storage = BoxProbe.demoStorage
    Card2(demo = true) {
        Text("Storage", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(8.dp))
        StorageMeter(storage.fraction, storage.phrase)
        Text(
            storage.label,
            style = MaterialTheme.typography.bodySmall,
            color = LocalTokens.current.scheme.onSurfaceVariant,
        )
    }
    Text(
        "Apps",
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(top = 10.dp, start = 4.dp),
    )
    AppTilesRow()
    Text(
        "Widgets",
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(top = 10.dp, start = 4.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(vertical = 6.dp)) {
        Widget("Photos", "1,208", "on the box", demo = true, modifier = Modifier.weight(1f))
        Widget("Board", "Chores", "3 left today", demo = true, modifier = Modifier.weight(1f))
        Widget("Mail", "42", "unread", demo = true, modifier = Modifier.weight(1f))
    }
    Card2(demo = true) {
        Text("Lent to the mesh", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(6.dp))
        Text(
            "1.2 TB spare disk, replicated. Spare CPU 18:00–22:00, only when idle.",
            style = MaterialTheme.typography.bodyMedium,
            color = LocalTokens.current.scheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun HealthCard(read: Read, config: BoxConfig) {
    val tokens = LocalTokens.current
    Card2 {
        Row(verticalAlignment = Alignment.CenterVertically) {
            val (state, dot) = when (val r = read) {
                Read.Loading -> "Asking…" to tokens.scheme.outline
                is Read.Failed -> "Box unreachable: ${r.reason}" to tokens.extra.warn
                is Read.Done ->
                    if (r.data.maybeBool("ok")) "Answering" to tokens.extra.ok
                    else "Not answering" to tokens.extra.crit
            }
            Box(Modifier
                .size(10.dp)
                .clip(CircleShape)
                .background(dot)) {}
            Spacer(Modifier.width(10.dp))
            Column {
                Text(state, style = MaterialTheme.typography.titleMedium)
                Text(
                    config.base,
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** The one card the Overview draws only when the box says it must. */
@Composable
private fun StatusCard(read: Read) {
    val tokens = LocalTokens.current
    if (read !is Read.Done) return
    val message = read.data.maybeString("message").orEmpty()
    when (read.data.maybeString("state")) {
        "building" -> Card2 {
            Text(
                "Applying changes",
                style = MaterialTheme.typography.titleMedium,
                color = tokens.extra.ok,
            )
            if (message.isNotEmpty()) {
                Spacer(Modifier.height(4.dp))
                Text(message, style = MaterialTheme.typography.bodyMedium)
            }
        }
        "failed" -> Card2 {
            Text(
                "The last change failed",
                style = MaterialTheme.typography.titleMedium,
                color = tokens.extra.crit,
            )
            if (message.isNotEmpty()) {
                Spacer(Modifier.height(4.dp))
                Text(message, style = MaterialTheme.typography.bodyMedium)
            }
        }
        else -> {}
    }
}

@Composable
internal fun StorageStripCard(read: Read) {
    val tokens = LocalTokens.current
    ReadCard("Storage", read) { d ->
        val used = d.maybeDouble("usedBytes")
        val total = d.maybeDouble("totalBytes")
        if (total != null && total > 0.0 && used != null) {
            LinearProgressIndicator(
                progress = { (used / total).toFloat().coerceIn(0f, 1f) },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(8.dp)
                    .clip(RoundedCornerShape(4.dp)),
                color = tokens.scheme.primary,
                trackColor = tokens.scheme.surfaceVariant,
            )
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text("${formatBytes(used)} on this box", style = MaterialTheme.typography.bodyMedium)
                Text(
                    "of ${formatBytes(total)} total",
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        } else {
            Text(
                "The disk has no size to report yet.",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun OverviewAppsCard(read: Read) {
    ReadCard("Apps", read) { d -> InstalledAppsBody(d, "Nothing installed yet.") }
}

/** The rows the Apps pane and the Overview both draw out of /api/apps. */
@Composable
internal fun InstalledAppsBody(d: JSONObject, emptyWord: String) {
    val tokens = LocalTokens.current
    if (!d.maybeBool("available")) {
        Text(
            "Apps do not run here while no storage is shared.",
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
        return
    }
    val records = d.itemsOf("apps")
    if (records.isEmpty()) {
        Text(
            emptyWord,
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
    }
    records.forEach { app -> InstalledAppRow(app) }
}

@Composable
internal fun InstalledAppRow(app: JSONObject) {
    val tokens = LocalTokens.current
    val release = app.maybeString("release") ?: "-"
    val phase = app.maybeString("phase") ?: "running"
    val chart = app.maybeObject("chart")
    val detail = buildList {
        add(chart?.maybeString("name") ?: "-")
        add(app.maybeString("runAs") ?: "notshared")
        if (phase == "failed") add(app.maybeString("message").orEmpty())
    }.filter { it.isNotEmpty() }.joinToString(" · ")
    PaneRow(
        title = release,
        detail = detail,
        value = phaseWord(phase),
        tone = when (phase) {
            "failed" -> tokens.extra.crit
            "running" -> tokens.extra.ok
            else -> tokens.scheme.onSurfaceVariant
        },
    )
}

/** What the pool gets from this box, straight out of /api/settings. */
@Composable
internal fun LendCard(read: Read) {
    val tokens = LocalTokens.current
    ReadCard("Lent to the mesh", read) { s ->
        val sharing = s.maybeBool("sharingMyStorage")
        val joined = s.maybeBool("clusterEnable")
        val compute = s.maybeBool("shareCompute")
        PaneRow(
            "Spare disk",
            detail = when {
                sharing && joined -> "Replicated into the pool."
                sharing -> "Waiting for a gateway of the box's cluster."
                else -> "Kept to this box alone."
            },
            value = if (sharing) "on" else "off",
            tone = if (sharing) tokens.extra.ok else tokens.scheme.onSurfaceVariant,
        )
        if (sharing) {
            val start = s.maybeString("computeWindowStart") ?: "00:00"
            val end = s.maybeString("computeWindowEnd") ?: "00:00"
            PaneRow(
                "Spare time",
                detail = describeWindow(start, end),
                value = if (compute) "lent" else "never",
                tone = if (compute) tokens.extra.ok else tokens.scheme.onSurfaceVariant,
            )
        }
        Text(
            "Lending is set on the box's own pages; a phone watches the result.",
            style = MaterialTheme.typography.bodySmall,
            color = tokens.scheme.onSurfaceVariant,
        )
    }
}

/** The meter and its phrase, as the dashboard's storage card has always drawn it. */
@Composable
internal fun StorageMeter(fraction: Float, phrase: String) {
    val tokens = LocalTokens.current
    LinearProgressIndicator(
        progress = { fraction.coerceIn(0f, 1f) },
        modifier = Modifier
            .fillMaxWidth()
            .height(8.dp)
            .clip(RoundedCornerShape(4.dp)),
        color = tokens.scheme.primary,
        trackColor = tokens.scheme.surfaceVariant,
    )
    Spacer(Modifier.height(8.dp))
    Text(phrase, style = MaterialTheme.typography.bodyMedium)
}

// ── Storage ──────────────────────────────────────────────────────────────

@Composable
fun StoragePane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Storage", "The disk as the box reports it", demo = true)
        val demo = BoxProbe.demoStorage
        Card2(demo = true) {
            Text("Disk", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            StorageMeter(demo.fraction, demo.phrase)
            Text(
                demo.label,
                style = MaterialTheme.typography.bodySmall,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Storage", "The disk as the box reports it", demo = false)
    ReadRoute("/api/storage", config, update) { read ->
        ReadCard("Disk", read) { d ->
            val tokens = LocalTokens.current
            val used = d.maybeDouble("usedBytes")
            val total = d.maybeDouble("totalBytes")
            if (total != null && total > 0.0 && used != null) {
                LinearProgressIndicator(
                    progress = { (used / total).toFloat().coerceIn(0f, 1f) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(8.dp)
                        .clip(RoundedCornerShape(4.dp)),
                    color = tokens.scheme.primary,
                    trackColor = tokens.scheme.surfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    "${formatBytes(used)} used of ${formatBytes(total)} · " +
                        "${formatBytes((total - used).coerceAtLeast(0.0))} free",
                    style = MaterialTheme.typography.bodyMedium,
                )
            } else {
                Text(
                    "The box does not report the disk's size yet.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        }
        ReadCard("Held in reserve", read) { d ->
            val reserve = d.maybeDouble("reserveBytes")
            PaneRow(
                "Reserve",
                detail = when {
                    reserve != null && reserve == 0.0 -> "Already spent — the disk is that much bigger now."
                    reserve != null -> "Unallocated disk the box can claim without a repair."
                    else -> "The box did not report a reserve."
                },
                value = formatBytes(reserve),
            )
            Text(
                "Spending it (POST /api/grow) stays on the box's own pages.",
                style = MaterialTheme.typography.bodySmall,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
    }
    Spacer(Modifier.height(20.dp))
}

// ── Mesh ─────────────────────────────────────────────────────────────────

@Composable
fun MeshPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Mesh", "The pool this box lends to and buys from", demo = true)
        Card2(demo = true) {
            Text("Edges in reach", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            Text(
                "The demo box sees no edges; sharing is off.",
                style = MaterialTheme.typography.bodyMedium,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Mesh", "The pool this box lends to and buys from", demo = false)
    ReadRoute("/api/edge", config, update) { read -> EdgeCard(read) }
    ReadRoute("/api/settings", config, update) { read -> LendCard(read) }
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun EdgeCard(read: Read) {
    val tokens = LocalTokens.current
    ReadCard("Edges in reach", read) { e ->
        if (!e.maybeBool("reachable")) {
            val detail = if (!e.maybeBool("lanSearched")) {
                "The LAN could not be searched."
            } else {
                e.maybeString("configuredUrl")?.let { "Tried $it; no answer." }
                    ?: "Nothing was tried: no registrar is configured."
            }
            PaneRow(
                "No edge answered",
                detail = detail,
                value = "sharing off",
                tone = tokens.extra.crit,
            )
            return@ReadCard
        }
        e.itemsOf("edges").forEach { edge ->
            val official = edge.maybeBool("official")
            val url = edge.maybeString("url")
            PaneRow(
                title = (edge.maybeString("name") ?: "an edge") + if (official) " ✓" else " !",
                detail = buildList {
                    if (url != null) add(url)
                    add(
                        if (official) "An official edge — it may process trading."
                        else "A company edge; it shares storage only."
                    )
                }.joinToString("\n"),
                value = if (edge.maybeString("source") == "configured") {
                    "over the internet"
                } else {
                    "found on the LAN"
                },
                tone = tokens.extra.ok,
            )
        }
        if (e.itemsOf("edges").isEmpty()) {
            Text(
                "The answer names no edge.",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
        Text(
            "Only an edge LosOS runs may process trading; any edge shares the pool.",
            style = MaterialTheme.typography.bodySmall,
            color = tokens.scheme.onSurfaceVariant,
        )
    }
}

// ── Apps ─────────────────────────────────────────────────────────────────

@Composable
fun AppsPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Apps", "What the box runs, and what it could", demo = true)
        AppTilesRow()
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Apps", "What the box runs, and what it could", demo = false)
    ReadRoute("/api/apps", config, update) { read -> InstalledAppsCard(read) }
    SearchCard(config, update)
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun InstalledAppsCard(read: Read) {
    ReadCard("Installed", read) { d ->
        InstalledAppsBody(d, "Nothing installed yet.")
    }
}

/** The catalogue search: GET /api/apps/search?q=…, as the SPA's search bar asks. */
@Composable
private fun SearchCard(config: BoxConfig, update: (BoxConfig) -> Unit) {
    val tokens = LocalTokens.current
    val api = boxApi(config, update)
    val scope = rememberCoroutineScope()
    var query by remember { mutableStateOf("") }
    var search by remember { mutableStateOf<Read?>(null) }

    fun find() {
        val term = query.trim()
        if (term.isEmpty()) return
        val asked = encodeQuery(term)
        scope.launch {
            search = Read.Loading
            search = try {
                Read.Done(api.get("/api/apps/search?q=$asked"))
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: ApiError) {
                Read.Failed(e.message ?: "HTTP ${e.status}")
            } catch (e: Exception) {
                Read.Failed(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    Card2 {
        Text("Find more", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(10.dp))
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Search the catalogue") },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { find() }),
        )
        Spacer(Modifier.height(8.dp))
        OutlinedButton(onClick = { find() }) { Text("Find") }
        when (val r = search) {
            null -> {}
            Read.Loading -> Text(
                "Looking…",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
            is Read.Failed -> Text(
                "Could not search: ${r.reason}",
                style = MaterialTheme.typography.bodySmall,
                color = tokens.extra.crit,
            )
            is Read.Done -> {
                val hits = r.data.itemsOf("results")
                if (hits.isEmpty()) {
                    Text(
                        "Nothing found.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = tokens.scheme.onSurfaceVariant,
                    )
                }
                hits.forEach { hit -> SearchHitRow(hit) }
                val sources = searchSources(r.data)
                Text(
                    "The box asked its ${sources.size} source${if (sources.size == 1) "" else "s"}; " +
                        "install and remove stay on its pages.",
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        }
    }
}

private fun searchSources(body: JSONObject): List<String> {
    val named = (body.opt("sources") as? org.json.JSONArray)?.let { arr ->
        (0 until arr.length()).mapNotNull { arr.opt(it) as? String }.filter { it.isNotBlank() }
    }.orEmpty()
    if (named.isNotEmpty()) return named
    return body.itemsOf("results")
        .mapNotNull { it.maybeString("source") }
        .distinct()
}

@Composable
private fun SearchHitRow(hit: JSONObject) {
    val tokens = LocalTokens.current
    val name = hit.maybeString("name") ?: "-"
    val version = hit.maybeString("version")
    val detail = buildList {
        add(hit.maybeString("summary").orEmpty())
        add("source: ${hit.maybeString("source") ?: "-"}")
    }.filter { it.isNotEmpty() }.joinToString("\n")
    PaneRow(
        title = if (version != null) "$name  $version" else name,
        detail = detail,
        tone = tokens.scheme.onSurface,
    )
}

// ── The demo tiles, kept from the dashboard's first drafts ───────────────

@Composable
internal fun AppTilesRow() {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(vertical = 6.dp)) {
        AppTile("Cloud", "LosOS Cloud", "1,204 files", demo = true, modifier = Modifier.weight(1f))
        AppTile("Git", "LosOS Git", "12 repos", demo = true, modifier = Modifier.weight(1f))
        AppTile("Admin", "Admin pages", "LAN only", demo = true, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun AppTile(glyph: String, name: String, sub: String, demo: Boolean, modifier: Modifier = Modifier) {
    val tokens = LocalTokens.current
    Card(
        modifier = modifier,
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(
            containerColor = tokens.scheme.surface,
            contentColor = tokens.scheme.onSurface,
        ),
    ) {
        Column(Modifier.padding(12.dp)) {
            Box(
                Modifier
                    .size(38.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(tokens.scheme.primaryContainer),
            ) {
                Text(
                    glyph,
                    modifier = Modifier.align(Alignment.Center),
                    style = MaterialTheme.typography.titleMedium,
                    color = tokens.scheme.onPrimaryContainer,
                )
            }
            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(name, style = MaterialTheme.typography.labelSmall)
                if (demo) {
                    Spacer(Modifier.width(4.dp))
                    DemoChip()
                }
            }
            Text(sub, style = MaterialTheme.typography.bodySmall, color = tokens.scheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun Widget(title: String, big: String, sub: String, demo: Boolean, modifier: Modifier = Modifier) {
    val tokens = LocalTokens.current
    Card(
        modifier = modifier,
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(
            containerColor = tokens.scheme.primaryContainer,
            contentColor = tokens.scheme.onPrimaryContainer,
        ),
    ) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(title, style = MaterialTheme.typography.labelSmall)
                if (demo) {
                    Spacer(Modifier.width(4.dp))
                    DemoChip()
                }
            }
            Text(big, style = MaterialTheme.typography.headlineSmall)
            Text(sub, style = MaterialTheme.typography.bodySmall)
        }
    }
}
