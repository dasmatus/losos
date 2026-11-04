package us.dasmat.losos.companion

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.material3.ColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/*
 * LosOS design tokens — a port of admin-ui/app/src/styles/tokens.css.
 * The admin SPA, LosOS Git, Nextcloud, LosOS Login and this companion share
 * one palette, so an owner moving from the box's homepage to the app should
 * not be able to tell where one ends and the next begins.
 */

data class OkWarnCrit(val ok: Color, val warn: Color, val crit: Color)

private val ExtraLight = OkWarnCrit(
    ok = Color(0xFF2E7357),
    warn = Color(0xFF8C6512),
    crit = Color(0xFFA8443C),
)

private val ExtraDark = OkWarnCrit(
    ok = Color(0xFF5FB894),
    warn = Color(0xFFCE9A44),
    crit = Color(0xFFD67C72),
)

data class Tokens(val scheme: ColorScheme, val extra: OkWarnCrit)

private val Light = Tokens(
    scheme = lightColorScheme(
        primary = Color(0xFF0E6E7D),
        onPrimary = Color(0xFFFFFFFF),
        primaryContainer = Color(0xFFE2EFF1),
        onPrimaryContainer = Color(0xFF0E6E7D),
        secondary = Color(0xFF2F5F9E),
        onSecondary = Color(0xFFFFFFFF),
        secondaryContainer = Color(0xFFE3EBF5),
        onSecondaryContainer = Color(0xFF2F5F9E),
        background = Color(0xFFF3F5F7),
        onBackground = Color(0xFF18212A),
        surface = Color(0xFFFFFFFF),
        onSurface = Color(0xFF18212A),
        surfaceVariant = Color(0xFFEAEDF0),
        onSurfaceVariant = Color(0xFF5C6A77),
        outline = Color(0xFFDCE1E6),
        outlineVariant = Color(0xFFEAEFF1),
        error = Color(0xFFA8443C),
    ),
    extra = ExtraLight,
)

private val Dark = Tokens(
    scheme = darkColorScheme(
        primary = Color(0xFF48B3C0),
        onPrimary = Color(0xFF06282D),
        primaryContainer = Color(0xFF0D2A2F),
        onPrimaryContainer = Color(0xFF48B3C0),
        secondary = Color(0xFF7FA6D2),
        onSecondary = Color(0xFF10253F),
        secondaryContainer = Color(0xFF12233B),
        onSecondaryContainer = Color(0xFF9DBBD8),
        background = Color(0xFF0A0E12),
        onBackground = Color(0xFFE2E8EE),
        surface = Color(0xFF141B22),
        onSurface = Color(0xFFE2E8EE),
        surfaceVariant = Color(0xFF10171D),
        onSurfaceVariant = Color(0xFF94A2B0),
        outline = Color(0xFF212C36),
        outlineVariant = Color(0xFF1A242D),
        error = Color(0xFFD67C72),
    ),
    extra = ExtraDark,
)

val LocalTokens = staticCompositionLocalOf { Light }

/** Theme mode the owner can pick: follow the system, or pin one. */
enum class ThemeMode { System, Light, Dark }

@Composable
fun lososTheme(mode: ThemeMode, content: @Composable () -> Unit) {
    val dark = when (mode) {
        ThemeMode.System -> isSystemInDarkTheme()
        ThemeMode.Light -> false
        ThemeMode.Dark -> true
    }
    val tokens = if (dark) Dark else Light
    androidx.compose.runtime.CompositionLocalProvider(LocalTokens provides tokens) {
        MaterialTheme(
            colorScheme = tokens.scheme,
            typography = lososTypography(),
            shapes = lososShapes(),
            content = content,
        )
    }
}

private fun lososTypography() = Typography(
    headlineMedium = TextStyle(
        fontWeight = FontWeight.SemiBold,
        fontSize = 26.sp,
        letterSpacing = (-0.2).sp,
    ),
    headlineSmall = TextStyle(
        fontWeight = FontWeight.SemiBold,
        fontSize = 21.sp,
        letterSpacing = (-0.1).sp,
    ),
    titleMedium = TextStyle(
        fontWeight = FontWeight.SemiBold,
        fontSize = 16.sp,
    ),
    bodyMedium = TextStyle(
        fontSize = 15.sp,
        lineHeight = 21.sp,
    ),
    bodySmall = TextStyle(
        fontSize = 13.sp,
        lineHeight = 18.sp,
    ),
    labelSmall = TextStyle(
        fontWeight = FontWeight.SemiBold,
        fontSize = 11.sp,
        letterSpacing = 0.5.sp,
    ),
)

private fun lososShapes() = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
)
