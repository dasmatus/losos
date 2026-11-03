package us.dasmat.losos.companion

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            var config by remember { mutableStateOf(BoxStore.load(this)) }
            val update: (BoxConfig) -> Unit = { config = it; BoxStore.save(this, it) }

            lososTheme(config.themeMode) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = LocalTokens.current.scheme.background,
                ) {
                    if (needsConnect(config)) {
                        ConnectScreen(config, update)
                    } else {
                        DashboardScreen(config, update)
                    }
                }
            }
        }
    }
}

/*
 * The connect screen stays until the demo is on, or a box is linked. A link
 * is an address *and* a token: when lososd throws the token back (a 401,
 * handled in ApiClient.kt) the token goes and the owner is asked again, the
 * way the admin SPA evicts (admin-ui/app/src/lib/api.ts).
 */
private fun needsConnect(config: BoxConfig) = when {
    config.demo -> config.host.isBlank()
    else -> config.host.isBlank() || config.token.isBlank()
}
