package com.kazui.delphi.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

private val DelphiDarkColorScheme = darkColorScheme(
    primary = AccentBlue,
    onPrimary = DarkBackground,
    primaryContainer = DarkCard,
    onPrimaryContainer = TextPrimary,
    secondary = AccentGreen,
    onSecondary = DarkBackground,
    background = DarkBackground,
    onBackground = TextPrimary,
    surface = DarkSurface,
    onSurface = TextPrimary,
    surfaceVariant = DarkCard,
    onSurfaceVariant = TextSecondary,
    error = AccentRed,
    onError = DarkBackground,
    outline = BorderColor,
    outlineVariant = BorderColor
)

@Composable
fun DelphiTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = DelphiDarkColorScheme,
        typography = DelphiTypography,
        content = content
    )
}
