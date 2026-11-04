package us.dasmat.losos.companion

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/*
 * The settings panes, as the terminal client's Settings screens and the web
 * UI's sidebars draw them. Every one reads its route read-only: a change to
 * these is a rebuild (POST /api/apply), which stays on the box's own pages;
 * the two actions a phone may honestly run, a config sync and a backup run,
 * are the History and Backup cards' buttons.
 */

// ── Network ──────────────────────────────────────────────────────────────

@Composable
fun NetworkPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Network", "The box's name, and how the world reaches it", demo = true)
        Card2(demo = true) {
            Text("Name", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            PaneRow("Called", value = "demo")
            PaneRow("Reached at", value = "demo.local")
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Network", "The box's name, and how the world reaches it", demo = false)
    ReadRoute("/api/settings", config, update) { read ->
        ReadCard("Name", read) { s ->
            val name = s.maybeString("hostName").orEmpty()
            PaneRow(
                "Called",
                value = name.ifEmpty { "not set" },
                detail = "A change to the name is a rebuild on the box.",
            )
            PaneRow("Reached at", value = if (name.isEmpty()) "-" else "$name.local")
        }
        ReadCard("Reaching the box", read) { s ->
            PaneRow(
                "From outside the home network",
                detail = "The reverse proxy the box can turn on for names beyond the LAN.",
                value = if (s.maybeBool("proxyEnable")) "yes" else "no",
            )
        }
    }
    Spacer(Modifier.height(20.dp))
}

// ── Security ─────────────────────────────────────────────────────────────

@Composable
fun SecurityPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Security", "Switches that cost something, each a rebuild", demo = true)
        Card2(demo = true) {
            Text("Hardening", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            SecurityRow("usbguard", true, "Blocks USB devices absent at boot.")
            SecurityRow("malloc", false, "Hardened malloc; host daemons only, not pods.")
            SecurityRow("apparmor", true, "Confines less than it looks like it does.")
            SecurityRow("nosmt", false, "Halves the core count.")
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Security", "Switches that cost something, each a rebuild", demo = false)
    ReadRoute("/api/settings", config, update) { read ->
        ReadCard("Hardening", read) { s ->
            SecurityRow("USB Guard", s.maybeBool("hardeningUsbguard"), "Blocks USB devices absent at boot.")
            SecurityRow("Hardened malloc", s.maybeBool("hardeningMalloc"), "Host daemons only, not pods.")
            SecurityRow("AppArmor", s.maybeBool("hardeningApparmor"), "Confines less than it looks like it does.")
            SecurityRow("No SMT", s.maybeBool("hardeningNosmt"), "Halves the core count.")
        }
        Card2 {
            Text(
                "The protections that cost nothing are always on and appear nowhere. " +
                    "Every switch here is off by default; turning one on is a rebuild.",
                style = MaterialTheme.typography.bodySmall,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
    }
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun SecurityRow(name: String, on: Boolean, detail: String) {
    val tokens = LocalTokens.current
    PaneRow(
        title = name,
        detail = detail,
        value = if (on) "on" else "off",
        tone = if (on) tokens.extra.ok else tokens.scheme.onSurfaceVariant,
    )
}

// ── Advanced: every option, read only ────────────────────────────────────

@Composable
fun AdvancedPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Advanced", "Every option the box declares, read only", demo = true)
        Card2(demo = true) {
            Text("Options", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            PaneRow("losos.demo.example", detail = "A stand-in for the box's options document.", value = "on")
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Advanced", "Every option the box declares, read only", demo = false)
    ReadRoute("/api/options", config, update) { read ->
        ReadCard("Options", read) { o ->
            if (!o.maybeBool("available")) {
                Text(
                    "The box has no options document right now.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = LocalTokens.current.scheme.onSurfaceVariant,
                )
                return@ReadCard
            }
            // Grouped as the Advanced pane groups them: general first, then the
            // rest in name order, hidden options left out (tui panes.rs).
            val groups = o.itemsOf("options")
                .filter { !it.maybeBool("hidden") }
                .groupBy { it.maybeString("group") ?: "general" }
                .toSortedMap(
                    compareBy<String> { it != "general" }.thenBy { it }
                )
            groups.forEach { (group, options) ->
                Text(
                    if (group == "general") "General" else "losos.$group",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                options.forEach { option -> OptionRow(option) }
            }
            val strays = o.itemsOf("stray")
            if (strays.isNotEmpty()) {
                Text(
                    "Stray assignments",
                    style = MaterialTheme.typography.titleMedium,
                    color = LocalTokens.current.extra.crit,
                    modifier = Modifier.padding(top = 10.dp),
                )
                strays.forEach { s ->
                    PaneRow(
                        "losos.${s.maybeString("key") ?: "-"}",
                        detail = "Overrides a line no option declares.",
                        value = s.maybeString("value").orEmpty(),
                        tone = LocalTokens.current.extra.crit,
                    )
                }
            }
            Text(
                "Read only on a phone: an option changes in the web UI's editor, or Apply from losos.",
                style = MaterialTheme.typography.bodySmall,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
    }
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun OptionRow(option: JSONObject) {
    val name = option.maybeString("name") ?: "-"
    val badges = buildList {
        if (option.maybeBool("danger")) add("careful")
        when (option.maybeString("fixed")) {
            "installer" -> add("set by the installer")
            null -> {}
            else -> add("fixed")
        }
        if (option.opt("set") != null && option.opt("set") != JSONObject.NULL) add("set")
    }
    val detail = buildList {
        add(option.maybeString("description").orEmpty())
        option.maybeString("defaultText")?.let { add("default: $it") }
    }.filter { it.isNotEmpty() }.joinToString("\n")
    PaneRow(
        title = "losos.$name" + (if (badges.isEmpty()) "" else "  [${badges.joinToString(", ")}]"),
        detail = detail,
        value = optionWord(option),
    )
}

/* A value in words, the Advanced pane's own describe_value (tui panes.rs). */
internal fun optionWord(option: JSONObject): String =
    describeValue(option.maybeObject("editor"), option.opt("current"))

private fun describeValue(editor: JSONObject?, value: Any?): String = when (value) {
    null, JSONObject.NULL -> "not set"
    is Boolean -> if (value) "on" else "off"
    is Number -> if (editor?.maybeString("kind") == "float" && value is Long) "$value.0" else "$value"
    is String -> if (value.isEmpty()) "\"\"" else value
    is JSONArray ->
        if (value.length() == 0) "not set"
        else (0 until value.length()).joinToString(", ") { describeValue(editor, value.opt(it)) }
    is JSONObject -> value.maybeString("package") ?: value.maybeString("nix").orEmpty()
    else -> "$value"
}

// ── History ──────────────────────────────────────────────────────────────

@Composable
fun HistoryPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("History", "The configuration repository, and every change in it", demo = true)
        Card2(demo = true) {
            Text("Commits", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            Text(
                "The demo box keeps no history.",
                style = MaterialTheme.typography.bodyMedium,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("History", "The configuration repository, and every change in it", demo = false)
    var refresh by remember { mutableIntStateOf(0) }
    ReadRoute("/api/config", config, update, refresh = refresh) { read ->
        HistoryCards(read, config, update, onDone = { refresh++ })
    }
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun HistoryCards(
    read: Read,
    config: BoxConfig,
    update: (BoxConfig) -> Unit,
    onDone: () -> Unit,
) {
    val tokens = LocalTokens.current
    ReadCard("Repository", read) { c ->
        val repo = c.maybeObject("repository")
        if (repo == null) {
            Text(
                "No configuration repository: the box keeps its history to itself.",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
            return@ReadCard
        }
        val url = repo.maybeString("url").orEmpty()
        PaneRow(
            "On LosOS Git",
            detail = "${repo.maybeString("owner") ?: "-"}/${repo.maybeString("name") ?: "-"}",
            value = config.base + url,
        )
        c.maybeObject("head")?.let { head ->
            PaneRow(
                "Branch",
                value = head.maybeString("branch") ?: "-",
                detail = "head ${shortSha(head.maybeString("sha"))}",
            )
        }
        PaneRow("Clone", detail = "${config.base}$url.git")
    }
    ReadCard("Sync", read) { c -> SyncCard(c, config, update, onDone) }
    ReadCard("Updates", read) { c ->
        val upstream = c.maybeObject("upstream")
        val url = upstream?.maybeString("url")
        if (upstream == null || url == null) {
            Text(
                "The box does not follow a published branch.",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
            return@ReadCard
        }
        PaneRow(
            "Source",
            detail = url,
            value = upstream.maybeString("branch") ?: "-",
            tone = tokens.scheme.primary,
        )
        upstream.maybeObject("report")?.let { report ->
            val remote = report.maybeString("remoteHead")?.let { "remote ${shortSha(it)}" }
            PaneRow(
                "Last look",
                detail = joinWords(
                    report.maybeString("detail").orEmpty(),
                    remote ?: "",
                ),
                value = report.maybeString("state") ?: "idle",
            )
        }
    }
    ReadCard("Commits", read) { c ->
        val log = c.itemsOf("log")
        if (log.isEmpty()) {
            Text(
                "No commits yet.",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
        log.forEach { entry ->
            PaneRow(
                entry.maybeString("subject") ?: "-",
                detail = formatStamp(entry.maybeString("when")),
                value = shortSha(entry.maybeString("sha")),
            )
        }
    }
}

@Composable
private fun SyncCard(
    c: JSONObject,
    config: BoxConfig,
    update: (BoxConfig) -> Unit,
    onDone: () -> Unit,
) {
    val tokens = LocalTokens.current
    val repo = c.maybeObject("repository")
    if (repo == null) {
        Text(
            "Nothing to sync: no repository.",
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
        return
    }
    val sync = c.maybeObject("sync")
    if (sync != null) {
        val remote = sync.maybeString("remoteHead")?.let { "remote ${shortSha(it)}" }
        PaneRow(
            "State",
            detail = joinWords(
                sync.maybeString("detail").orEmpty(),
                remote ?: "",
            ),
            value = sync.maybeString("state") ?: "idle",
        )
    }
    val api = boxApi(config, update)
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf<String?>(null) }
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Button(
            onClick = {
                busy = true
                failed = null
                scope.launch {
                    try {
                        api.post("/api/config/sync")
                        onDone()
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: Exception) {
                        failed = e.message ?: e.javaClass.simpleName
                    } finally {
                        busy = false
                    }
                }
            },
            enabled = !busy,
        ) { Text(if (busy) "Syncing…" else "Sync now") }
    }
    failed?.let {
        Spacer(Modifier.height(6.dp))
        Text(
            "Could not sync: $it",
            style = MaterialTheme.typography.bodySmall,
            color = tokens.extra.crit,
        )
    }
}

// ── Backup ───────────────────────────────────────────────────────────────

@Composable
fun BackupPane(config: BoxConfig, update: (BoxConfig) -> Unit) {
    if (config.demo) {
        PaneHeader("Backup", "Where the box copies itself, and the way back", demo = true)
        Card2(demo = true) {
            Text("Target", style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            Text(
                "The demo box keeps no backups.",
                style = MaterialTheme.typography.bodyMedium,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.height(20.dp))
        return
    }
    PaneHeader("Backup", "Where the box copies itself, and the way back", demo = false)
    var refresh by remember { mutableIntStateOf(0) }
    ReadRoute("/api/backup", config, update, refresh = refresh) { read ->
        ReadCard("Target", read) { b ->
            val tokens = LocalTokens.current
            val target = b.maybeObject("target")
            if (target == null) {
                Text(
                    "No target yet: nothing is copied anywhere.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = tokens.scheme.onSurfaceVariant,
                )
                return@ReadCard
            }
            val bucket = target.maybeString("bucket") ?: "-"
            val prefix = target.maybeString("prefix").orEmpty()
            PaneRow("Endpoint", value = target.maybeString("endpoint") ?: "-")
            PaneRow("Bucket", value = if (prefix.isEmpty()) bucket else "$bucket/$prefix")
            val storageClass = target.maybeString("storageClass") ?: "standard"
            PaneRow("Class", value = storageClass)
            PaneRow("Key", value = target.maybeString("accessKeyId") ?: "-")
            Text(
                "The secret key stays sealed on the box.",
                style = MaterialTheme.typography.bodySmall,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
        ReadCard("Runs", read) { b ->
            val target = b.maybeObject("target") != null
            val job = b.maybeObject("job")?.takeIf { it.maybeString("kind") == "backup" }
            val jobState = job?.maybeString("state")
            val last = b.maybeObject("last")
            PaneRow(
                "Last run",
                detail = if (last != null) {
                    "${formatBytes(last.maybeDouble("bytes"))} · " +
                        "${(last.maybeDouble("files") ?: 0.0).toInt()} files"
                } else {
                    "Never"
                },
                value = formatUnix(last?.maybeNumber("time")),
            )
            if (jobState == "running") {
                Text("Copying now…", style = MaterialTheme.typography.bodyMedium,
                    color = LocalTokens.current.scheme.primary)
            }
            if (jobState == "failed") {
                Text(
                    "The last run failed: ${job?.maybeString("message").orEmpty()}",
                    style = MaterialTheme.typography.bodySmall,
                    color = LocalTokens.current.extra.crit,
                )
            }
            BackupRunButton(target, jobState, config, update) { refresh++ }
            Text(
                "Restores live in the web UI.",
                style = MaterialTheme.typography.bodySmall,
                color = LocalTokens.current.scheme.onSurfaceVariant,
            )
        }
    }
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun BackupRunButton(
    hasTarget: Boolean,
    jobState: String?,
    config: BoxConfig,
    update: (BoxConfig) -> Unit,
    onDone: () -> Unit,
) {
    val tokens = LocalTokens.current
    val api = boxApi(config, update)
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf<String?>(null) }
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Button(
            onClick = {
                busy = true
                failed = null
                scope.launch {
                    try {
                        api.post("/api/backup/run")
                        onDone()
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: Exception) {
                        failed = e.message ?: e.javaClass.simpleName
                    } finally {
                        busy = false
                    }
                }
            },
            enabled = hasTarget && jobState != "running" && !busy,
        ) { Text(if (busy) " Asking…" else "Copy now") }
    }
    failed?.let {
        Spacer(Modifier.height(6.dp))
        Text(
            "Could not start: $it",
            style = MaterialTheme.typography.bodySmall,
            color = tokens.extra.crit,
        )
    }
}

internal fun joinWords(vararg parts: String): String =
    parts.filter { it.isNotEmpty() }.joinToString(" · ")
