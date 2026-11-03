package us.dasmat.losos.companion

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            var config by remember { mutableStateOf(BoxStore.load(this)) }
            val scope = androidx.compose.runtime.rememberCoroutineScope()
            val update: (BoxConfig) -> Unit = { config = it; BoxStore.save(this, it) }

            lososTheme(config.themeMode) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = LocalTokens.current.scheme.background,
                ) {
                    if (config.host.isBlank() && config.demo) {
                        ConnectScreen(config, update) { linked ->
                            update(linked)
                            if (!linked.demo) {
                                scope.launch {
                                    // Link verified below on the dashboard probe.
                                }
                            }
                        }
                    } else {
                        DashboardScreen(config, update)
                    }
                }
            }
        }
    }
}
