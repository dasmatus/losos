package us.dasmat.losos.companion

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

/*
 * The shell: one bar with the box's panes, in the phone's order. Each tab is
 * a pane over the routes the admin SPA drives (Panes.kt, SettingsPanes.kt);
 * eleven of them scroll, the way a phone carries the terminal client's panes.
 */

@Composable
fun DashboardScreen(config: BoxConfig, update: (BoxConfig) -> Unit) {
    val tabs = listOf(
        "Dashboard", "Storage", "Mesh", "Apps", "Network",
        "Security", "Advanced", "History", "Backup", "Cloud", "Settings",
    )
    var current by remember { mutableIntStateOf(0) }
    Scaffold(
        bottomBar = { PaneBar(tabs, current) { current = it } },
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
        ) {
            when (current) {
                0 -> OverviewPane(config, update)
                1 -> StoragePane(config, update)
                2 -> MeshPane(config, update)
                3 -> AppsPane(config, update)
                4 -> NetworkPane(config, update)
                5 -> SecurityPane(config, update)
                6 -> AdvancedPane(config, update)
                7 -> HistoryPane(config, update)
                8 -> BackupPane(config, update)
                9 -> CloudTab(config, update)
                10 -> SettingsTab(config, update)
            }
        }
    }
}

/* The tab bar wears the palette like a NavigationBar but scrolls: the TUI's
 * panes number more than a thumb's width holds at once. */
@Composable
private fun PaneBar(tabs: List<String>, current: Int, pick: (Int) -> Unit) {
    val tokens = LocalTokens.current
    Surface(color = tokens.scheme.surface, tonalElevation = 6.dp) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 6.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            tabs.forEachIndexed { index, name ->
                val selected = current == index
                Column(
                    modifier = Modifier
                        .clip(RoundedCornerShape(24.dp))
                        .background(
                            if (selected) tokens.scheme.secondaryContainer
                            else Color.Transparent
                        )
                        .clickable { pick(index) }
                        .padding(horizontal = 14.dp, vertical = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(
                        modifier = Modifier
                            .size(22.dp)
                            .clip(CircleShape)
                            .background(
                                if (selected) tokens.scheme.primary
                                else tokens.scheme.outline,
                            ),
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        name,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (selected) tokens.scheme.onSecondaryContainer
                                else tokens.scheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
fun Card2(
    demo: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    val tokens = LocalTokens.current
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 6.dp),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(
            containerColor = tokens.scheme.surface,
            contentColor = tokens.scheme.onSurface,
        ),
    ) {
        Column(Modifier.padding(16.dp)) {
            if (demo) {
                DemoChip(Modifier.align(Alignment.End).padding(bottom = 6.dp))
            }
            content()
        }
    }
}

@Composable
fun CloudTab(config: BoxConfig, update: (BoxConfig) -> Unit) {
    Row(
        Modifier.padding(vertical = 12.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("LosOS Cloud", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.weight(1f))
        if (config.demo) DemoChip()
    }
    Text(
        "Your files, photos, calendars and boards live here.",
        style = MaterialTheme.typography.bodyMedium,
        color = LocalTokens.current.scheme.onSurfaceVariant,
        modifier = Modifier.padding(bottom = 6.dp, start = 4.dp),
    )
    if (config.demo) {
        Card2(demo = true) {
            CloudRow("Documents", "108 files · 2.3 GB")
            CloudRow("Photos", "1,208 · last import today")
            CloudRow("Music", "213 · 1.1 GB")
            CloudRow("Boards", "Chore chart · 3 left today")
        }
    } else {
        ReadRoute("/api/cloud/files?path=%2F", config, update) { read ->
            CloudFilesCard(read, "Files")
        }
    }
    Card2(demo = config.demo) {
        Text("Folders kept on the box only:", style = MaterialTheme.typography.labelSmall)
        Spacer(Modifier.height(4.dp))
        Text(
            "Deleted from /persist the day you wipe the box, kept in every daily snapshot.",
            style = MaterialTheme.typography.bodyMedium,
        )
    }
    Spacer(Modifier.height(20.dp))
}

/** GET /api/cloud/files?path=…, the root listing, folders first. */
@Composable
private fun CloudFilesCard(read: Read, title: String) {
    val tokens = LocalTokens.current
    ReadCard(title, read) { c ->
        if (c.maybeBool("available")) {
            val entries = c.itemsOf("entries")
            if (entries.isEmpty()) {
                Text(
                    "Nothing at the top yet.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
            entries.forEach { entry ->
                CloudRow(
                    name = entry.maybeString("name") ?: "-",
                    meta = if (entry.maybeBool("folder")) {
                        "folder"
                    } else {
                        formatBytes(entry.maybeDouble("size"))
                    },
                )
            }
            if (c.maybeBool("more")) {
                Text(
                    "There are more; deeper browsing stays in LosOS Cloud for now.",
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        } else {
            Text(
                when (c.maybeString("reason")) {
                    "signIn" -> "Your files unlock the first time you sign in on the box's pages."
                    else -> "LosOS Cloud is not answering right now."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun CloudRow(name: String, meta: String) {
    val tokens = LocalTokens.current
    Row(
        Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .size(34.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(tokens.scheme.surfaceVariant)
                .padding(8.dp),
        ) {
            Box(Modifier.fillMaxWidth().height(2.dp).background(tokens.scheme.outline))
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.titleMedium)
            Text(meta, style = MaterialTheme.typography.bodySmall, color = tokens.scheme.onSurfaceVariant)
        }
    }
}

@Composable
fun SettingsTab(config: BoxConfig, update: (BoxConfig) -> Unit) {
    val tokens = LocalTokens.current
    var host by remember(config.host) { mutableStateOf(config.host) }
    var token by remember(config.token) { mutableStateOf(config.token) }
    var probe by remember { mutableStateOf<ProbeResult?>(null) }
    val scope = rememberCoroutineScope()

    Row(
        Modifier.padding(vertical = 12.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("Settings", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.weight(1f))
        if (config.demo) DemoChip()
    }

    Card2(demo = config.demo) {
        Text("Box", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(10.dp))
        OutlinedTextField(
            value = host,
            onValueChange = { host = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Address") },
        )
        Spacer(Modifier.height(8.dp))
        OutlinedTextField(
            value = token,
            onValueChange = { token = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Access token") },
        )
        Spacer(Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OutlinedButton(onClick = {
                probe = null
                update(config.copy(host = host.trim(), token = token.trim(), demo = config.demo))
            }) { Text("Save") }
            Button(onClick = {
                val candidate = config.copy(host = host.trim(), token = token.trim(), demo = config.demo)
                update(candidate)
                // Reachability, not identity: the probe carries no token (BoxStore.kt).
                scope.launch { probe = BoxProbe.probe(candidate) }
            }) { Text("Probe now") }
        }
        probe?.let { result ->
            Spacer(Modifier.height(8.dp))
            val text = if (result is ProbeResult.Reachable) {
                "Box answered in ${result.ms} ms"
            } else {
                "No answer: ${(result as ProbeResult.Unreachable).reason}"
            }
            Text(
                text,
                style = MaterialTheme.typography.bodySmall,
                color = if (result is ProbeResult.Reachable) tokens.extra.ok else tokens.scheme.onSurfaceVariant,
            )
        }
    }

    Card2(demo = config.demo) {
        Text("Theme", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ThemeMode.entries.forEach { mode ->
                val colors = FilterChipDefaults.filterChipColors(
                    containerColor = tokens.scheme.surfaceVariant,
                    labelColor = tokens.scheme.onSurfaceVariant,
                )
                FilterChip(
                    selected = config.themeMode == mode,
                    onClick = { update(config.copy(themeMode = mode)) },
                    label = { Text(mode.name) },
                    colors = colors,
                )
            }
        }
    }

    Card2(demo = config.demo) {
        Text("About", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(4.dp))
        Text(
            "LosOS Companion 0.1.0 — the box's panes in a phone's pocket, the same " +
                "routes the admin pages and the losos terminal read. The destructive " +
                "two, factory reset and erase, stay in the web UI.",
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
    }
    Spacer(Modifier.height(20.dp))
}
