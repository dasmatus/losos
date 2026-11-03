package us.dasmat.losos.companion

import androidx.compose.foundation.Image
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
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
    val context = LocalContext.current
    var host by remember { mutableStateOf(config.host.takeIf { it.isNotBlank() } ?: "box.local") }
    var token by remember { mutableStateOf(config.token) }
    var probe by remember { mutableStateOf<ProbeResult>(ProbeResult.Unreachable("Not probed yet")) }
    // The LAN's boxes by their `_losos._tcp` announcement (Discovery.kt);
    // null until the owner asks, empty when the browse came back with none.
    var boxes by remember { mutableStateOf<List<FoundBox>?>(null) }
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

        Spacer(Modifier.height(24.dp))
        Text("On this network", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(6.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = {
                boxes = null
                scope.launch { boxes = discoverBoxes(context) }
            }) { Text(if (boxes == null) "Find boxes" else "Browse again") }
        }
        when (val found = boxes) {
            null -> {}
            else -> if (found.isEmpty()) {
                Spacer(Modifier.height(6.dp))
                Text(
                    "None announced on this network — a box that is off, or a LAN no box answers on.",
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            } else {
                found.forEach { box ->
                    Spacer(Modifier.height(6.dp))
                    val address = when (box.port) {
                        443 -> box.address
                        80 -> "http://${box.address}"
                        else -> "https://${box.address}:${box.port}"
                    }
                    BoxRow(
                        name = box.address,
                        detail = listOfNotNull(
                            box.version?.let { "LosOS $it" },
                        ).joinToString(" · "),
                        onClick = { host = address },
                        // Phones whose own .local lookup is broken reach the
                        // box by the address resolution gave the browse.
                        fallback = box.ip?.let { ip ->
                            { host = if (box.port == 443) ip else "https://$ip:${box.port}" }
                        },
                    )
                }
            }
        }
    }
}

/** The LosOS brand: the coho salmon of themes/brand/salmon.png (NOAA
 *  Fisheries cutout, credits beside the original), the mark every front
 *  end carries. */
@Composable
fun Brand(modifier: Modifier = Modifier) {
    val tokens = LocalTokens.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Image(
            painterResource(R.drawable.salmon),
            contentDescription = "LosOS — a coho salmon",
            modifier = Modifier.size(52.dp),
            contentScale = ContentScale.Fit,
        )
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

/** One answer of the browse: the box's `.local` name with its release tag;
 *  tapping takes it into the address, `fallback` (an `IP` pill, present when
 *  resolution answered) takes the address the LAN gave when the phone's own
 *  `.local` lookup is broken. */
@Composable
private fun BoxRow(name: String, detail: String, onClick: () -> Unit, fallback: (() -> Unit)? = null) {
    val tokens = LocalTokens.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(tokens.scheme.surfaceVariant)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.titleMedium)
            if (detail.isNotBlank()) {
                Text(
                    detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = tokens.scheme.onSurfaceVariant,
                )
            }
        }
        fallback?.let {
            Text(
                "IP",
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .clickable(onClick = it)
                    .padding(horizontal = 10.dp, vertical = 4.dp),
                style = MaterialTheme.typography.labelSmall,
                color = tokens.scheme.primary,
            )
        }
    }
}
