package us.dasmat.losos.companion

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch

/**
 * The first screen until the companion knows a box (or the owner picks the
 * demo). Fields mirror the terminal client's `losos login <address>`; the
 * pasted token is the one the API client rides on (ApiClient.kt), and the
 * same screen is where a 401 lands the owner when the box rejects the old
 * one.
 */
@Composable
fun ConnectScreen(
    config: BoxConfig,
    update: (BoxConfig) -> Unit,
) {
    val tokens = LocalTokens.current
    var host by remember { mutableStateOf(config.host.takeIf { it.isNotBlank() } ?: "box.local") }
    var token by remember { mutableStateOf(config.token) }
    var probe by remember { mutableStateOf<ProbeResult>(ProbeResult.Unreachable("Not probed yet")) }
    val scope = rememberCoroutineScope()
    // The box threw the token back: this screen is the re-prompt now.
    val unlocked = config.host.isNotBlank() && config.token.isBlank()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(28.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Brand()
        Spacer(Modifier.height(36.dp))
        Text(
            if (unlocked) "The box wants its token again" else "Find your box",
            style = MaterialTheme.typography.headlineSmall,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            if (unlocked) "The one you pasted no longer opens it — paste its current one."
            else "Reach it as <name>.local on the home network, or by the IP printed on the box's screen.",
            style = MaterialTheme.typography.bodyMedium,
            color = tokens.scheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(20.dp))
        OutlinedTextField(
            value = host,
            onValueChange = { host = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Box address") },
            supportingText = { Text("box.local or 192.168.1.42") },
        )
        Spacer(Modifier.height(12.dp))
        OutlinedTextField(
            value = token,
            onValueChange = { token = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Access token") },
            supportingText = { Text("Settings → Access tokens in the admin pages") },
        )
        Spacer(Modifier.height(20.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = {
                scope.launch {
                    // Reachability only: the probe never carries the token.
                    probe = BoxProbe.probe(config.copy(host = host.trim(), token = token.trim()))
                }
            }) { Text("Probe") }
            Button(onClick = {
                update(config.copy(host = host.trim(), token = token.trim(), demo = false))
            }) { Text("Link box") }
        }
        Spacer(Modifier.height(14.dp))
        OutlinedButton(onClick = {
            update(config.copy(host = host.trim(), token = token.trim(), demo = true))
        }) { Text("Look around in demo mode") }
        Spacer(Modifier.height(18.dp))
        when (val p = probe) {
            is ProbeResult.Reachable ->
                Text("Box answered in ${p.ms} ms", style = MaterialTheme.typography.bodyMedium,
                    color = tokens.extra.ok)
            is ProbeResult.Unreachable ->
                Text("No answer: ${p.reason}", style = MaterialTheme.typography.bodyMedium,
                    color = tokens.scheme.onSurfaceVariant)
            ProbeResult.Probing ->
                Text("Probing…", style = MaterialTheme.typography.bodyMedium)
        }

        Spacer(Modifier.height(26.dp))
        BoxesNearby(onPick = { host = it })
    }
}

/**
 * What the LAN answers with: the boxes announcing `_losos._tcp` (Discovery.kt),
 * the record the box publishes and LosOS Desktop lists boxes by. The browse
 * starts when the screen shows and again on the button; a tap drops the box's
 * `<name>.local` into the address field — the token stays yours to paste.
 */
@Composable
private fun BoxesNearby(onPick: (String) -> Unit) {
    val tokens = LocalTokens.current
    val context = LocalContext.current
    var boxes by remember { mutableStateOf<List<FoundBox>?>(null) }
    var round by remember { mutableIntStateOf(0) }

    LaunchedEffect(round) {
        boxes = null
        boxes = discoverBoxes(context)
    }

    Card2 {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Boxes on your network", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Every box announces itself as <name>.local",
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
            OutlinedButton(onClick = { round++ }) { Text("Search again") }
        }
        Spacer(Modifier.height(6.dp))
        when (val found = boxes) {
            null -> Text(
                "Listening for boxes…",
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.scheme.onSurfaceVariant,
            )
            else -> if (found.isEmpty()) {
                Text(
                    "None answered. Is the phone on the box's home network?",
                    style = MaterialTheme.typography.bodyMedium,
                    color = tokens.scheme.onSurfaceVariant,
                )
            } else {
                found.forEach { box -> FoundBoxRow(box, onPick) }
            }
        }
    }
}

@Composable
private fun FoundBoxRow(box: FoundBox, onPick: (String) -> Unit) {
    val tokens = LocalTokens.current
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable { onPick(box.address) }
            .padding(vertical = 8.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(box.name, style = MaterialTheme.typography.titleMedium)
            Text(
                listOfNotNull(box.version, box.ip, "port ${box.port}").joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = tokens.scheme.onSurfaceVariant,
            )
        }
        Text(
            "use",
            style = MaterialTheme.typography.labelSmall,
            color = tokens.scheme.primary,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

/** The LosOS brand plate: the salmon field, the box's homepage mark in miniature. */
@Composable
fun Brand(modifier: Modifier = Modifier) {
    val tokens = LocalTokens.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(
            modifier = Modifier
                .size(56.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(Color(0xFFE8756A)),
        ) {
            Text(
                "L",
                modifier = Modifier.align(Alignment.Center),
                style = MaterialTheme.typography.headlineMedium,
                color = Color.White,
                fontWeight = FontWeight.Bold,
            )
        }
        Spacer(Modifier.width(14.dp))
        Column {
            Text("LosOS", style = MaterialTheme.typography.headlineSmall)
            Text(
                "Companion",
                style = MaterialTheme.typography.titleMedium,
                color = tokens.scheme.primary,
            )
        }
    }
}
