package us.dasmat.losos.companion

import androidx.compose.foundation.background
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
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch

/**
 * The first screen until the companion knows a box (or the owner picks the
 * demo). Fields mirror the terminal client's `losos login <address>`.
 */
@Composable
fun ConnectScreen(
    config: BoxConfig,
    update: (BoxConfig) -> Unit,
    onLink: (BoxConfig) -> Unit,
) {
    val tokens = LocalTokens.current
    var host by remember { mutableStateOf(config.host.takeIf { it.isNotBlank() } ?: "box.local") }
    var token by remember { mutableStateOf(config.token) }
    var probe by remember { mutableStateOf<ProbeResult>(ProbeResult.Unreachable("Not probed yet")) }
    val scope = rememberCoroutineScope()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(28.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Brand()
        Spacer(Modifier.height(36.dp))
        Text("Find your box", style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(4.dp))
        Text(
            "Reach it as <name>.local on the home network, or by the IP printed on the box's screen.",
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
                    probe = BoxProbe.probe(config.copy(host = host.trim(), token = token.trim()))
                }
            }) { Text("Probe") }
            Button(onClick = {
                val linked = config.copy(host = host.trim(), token = token.trim(), demo = false)
                update(linked)
                onLink(linked)
            }) { Text("Link box") }
        }
        Spacer(Modifier.height(14.dp))
        OutlinedButton(onClick = {
            val demo = config.copy(host = host.trim(), token = token.trim(), demo = true)
            update(demo)
            onLink(demo)
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
