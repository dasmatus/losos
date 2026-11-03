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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp

/*
 * The one screen that mirrors the box's admin homepage: the connection
 * card, the storage card, the app tiles and the widget board — and the
 * two other tabs of the phone shape, Cloud and Settings.
 */

@Composable
fun DashboardScreen(config: BoxConfig, update: (BoxConfig) -> Unit) {
    val tabs = listOf("Dashboard", "Cloud", "Settings")
    var current by remember { mutableIntStateOf(0) }
    Scaffold(
        bottomBar = {
            NavigationBar(containerColor = LocalTokens.current.scheme.surface) {
                tabs.forEachIndexed { index, name ->
                    NavigationBarItem(
                        selected = current == index,
                        onClick = { current = index },
                        label = { Text(name) },
                        icon = {
                            Box(
                                modifier = Modifier
                                    .size(22.dp)
                                    .clip(CircleShape)
                                    .background(
                                        if (current == index) LocalTokens.current.scheme.primary
                                        else LocalTokens.current.scheme.outline,
                                    ),
                            )
                        },
                    )
                }
            }
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
        ) {
            when (current) {
                0 -> DashboardTab(config)
                1 -> CloudTab(config)
                2 -> SettingsTab(config, update)
                else -> {}
            }
        }
    }
}

@Composable
fun Card2(content: @Composable () -> Unit) {
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
        Column(Modifier.padding(16.dp)) { content() }
    }
}

@Composable
fun DashboardTab(config: BoxConfig) {
    val tokens = LocalTokens.current
    var probe by remember(config.host) { mutableStateOf<ProbeResult?>(null) }
    LaunchedEffect(config.host, config.demo) {
        if (!config.demo && config.host.isNotBlank()) probe = BoxProbe.probe(config)
    }

    Row(
        Modifier.padding(vertical = 12.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("Your box", style = MaterialTheme.typography.headlineMedium)
            val subtitle = if (config.demo && config.host.isBlank()) {
                "Demo data — nothing phones home"
            } else {
                config.host
            }
            Text(
                subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
    }

    Card2 {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .size(10.dp)
                    .clip(CircleShape)
                    .background(
                        if (probe is ProbeResult.Reachable || config.demo) tokens.extra.ok
                        else tokens.extra.warn,
                    ),
            )
            Spacer(Modifier.width(10.dp))
            Column {
                val state = when {
                    config.demo -> "Demo mode"
                    probe is ProbeResult.Reachable -> "Linked · ${(probe as ProbeResult.Reachable).ms} ms"
                    probe is ProbeResult.Unreachable -> "Box unreachable"
                    else -> "Probing…"
                }
                Text(state, style = MaterialTheme.typography.titleMedium)
                Text(
                    config.base,
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        }
    }

    val storage = BoxProbe.demoStorage
    Card2 {
        Text("Storage", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(8.dp))
        LinearProgressIndicator(
            progress = { storage.fraction },
            modifier = Modifier
                .fillMaxWidth()
                .height(8.dp)
                .clip(RoundedCornerShape(4.dp)),
            color = tokens.scheme.primary,
            trackColor = tokens.scheme.surfaceVariant,
        )
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(storage.phrase, style = MaterialTheme.typography.bodyMedium)
            Text(
                storage.label,
                style = MaterialTheme.typography.bodySmall,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
    }

    Text(
        "Apps",
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(top = 10.dp, start = 4.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(vertical = 6.dp)) {
        AppTile("Cloud", "LosOS Cloud", "1,204 files", Modifier.weight(1f))
        AppTile("Git", "LosOS Git", "12 repos", Modifier.weight(1f))
        AppTile("Admin", "Admin pages", "LAN only", Modifier.weight(1f))
    }

    Text(
        "Widgets",
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(top = 10.dp, start = 4.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(vertical = 6.dp)) {
        Widget("Photos", "1,208", "on the box", Modifier.weight(1f))
        Widget("Board", "Chores", "3 left today", Modifier.weight(1f))
        Widget("Mail", "42", "unread", Modifier.weight(1f))
    }

    Card2 {
        Text("Lent to the mesh", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(6.dp))
        Text(
            "1.2 TB spare disk, replicated. Spare CPU 18:00–22:00, only when idle.",
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
    }
    Spacer(Modifier.height(20.dp))
}

@Composable
private fun AppTile(glyph: String, name: String, sub: String, modifier: Modifier = Modifier) {
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
            Text(name, style = MaterialTheme.typography.labelSmall)
            Text(sub, style = MaterialTheme.typography.bodySmall, color = tokens.scheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun Widget(title: String, big: String, sub: String, modifier: Modifier = Modifier) {
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
            Text(title, style = MaterialTheme.typography.labelSmall)
            Text(big, style = MaterialTheme.typography.headlineSmall)
            Text(sub, style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
fun CloudTab(config: BoxConfig) {
    val tokens = LocalTokens.current
    Row(
        Modifier.padding(vertical = 12.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("LosOS Cloud", style = MaterialTheme.typography.headlineMedium)
    }
    Text(
        "Your files, photos, calendars and boards live here; the Nextcloud apps sync with the box too.",
        style = MaterialTheme.typography.bodyMedium,
        color = tokens.scheme.onSurfaceVariant,
        modifier = Modifier.padding(bottom = 6.dp, start = 4.dp),
    )
    Card2 {
        CloudRow("Documents", "108 files · 2.3 GB")
        CloudRow("Photos", "1,208 · last import today")
        CloudRow("Music", "213 · 1.1 GB")
        CloudRow("Boards", "Chore chart · 3 left today")
    }
    Card2 {
        Text("Folders kept on the box only:", style = MaterialTheme.typography.labelSmall)
        Spacer(Modifier.height(4.dp))
        Text(
            "Documents, Photos, Music — deleted from /persist the day you wipe the box, kept in every daily snapshot.",
            style = MaterialTheme.typography.bodyMedium,
        )
    }
    Spacer(Modifier.height(20.dp))
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

    Row(
        Modifier.padding(vertical = 12.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("Settings", style = MaterialTheme.typography.headlineMedium)
    }

    Card2 {
        Text("Box", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(10.dp))
        androidx.compose.material3.OutlinedTextField(
            value = host,
            onValueChange = { host = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Address") },
        )
        Spacer(Modifier.height(8.dp))
        androidx.compose.material3.OutlinedTextField(
            value = token,
            onValueChange = { token = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Access token") },
        )
        Spacer(Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            androidx.compose.material3.OutlinedButton(onClick = {
                probe = null
                update(config.copy(host = host.trim(), token = token.trim(), demo = config.demo))
            }) { Text("Save") }
            androidx.compose.material3.Button(onClick = {
                update(config.copy(host = host.trim(), token = token.trim(), demo = config.demo))
                kotlinx.coroutines.runBlocking { probe = BoxProbe.probe(config.copy(host = host.trim(), token = token.trim())) }
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

    Card2 {
        Text("Theme", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ThemeMode.entries.forEach { mode ->
                val label = mode.name
                val colors = androidx.compose.material3.FilterChipDefaults.filterChipColors(
                    containerColor = tokens.scheme.surfaceVariant,
                    labelColor = tokens.scheme.onSurfaceVariant,
                )
                androidx.compose.material3.FilterChip(
                    selected = config.themeMode == mode,
                    onClick = { update(config.copy(themeMode = mode)) },
                    label = { Text(label) },
                    colors = colors,
                )
            }
        }
    }

    Card2 {
        Text("About", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(4.dp))
        Text(
            "LosOS Companion 0.1.0 — the box in a phone's pocket. Built from the LosOS admin family design tokens; the palette here is the one palette three front ends share.",
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
    }
    Spacer(Modifier.height(20.dp))
}
