package com.kazui.delphi.ui.components

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.kazui.delphi.data.sync.SyncStatus
import com.kazui.delphi.ui.theme.AccentGreen
import com.kazui.delphi.ui.theme.AccentOrange
import com.kazui.delphi.ui.theme.AccentRed

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConnectionIndicator(status: SyncStatus, modifier: Modifier = Modifier) {
    val color = when (status) {
        SyncStatus.ONLINE -> AccentGreen
        SyncStatus.SYNCING -> AccentOrange
        SyncStatus.OFFLINE -> AccentRed
    }
    val label = when (status) {
        SyncStatus.ONLINE -> "Синхронизировано"
        SyncStatus.SYNCING -> "Подключение..."
        SyncStatus.OFFLINE -> "Не подключено"
    }

    val alpha by if (status == SyncStatus.SYNCING) {
        val infiniteTransition = rememberInfiniteTransition(label = "pulse")
        infiniteTransition.animateFloat(
            initialValue = 0.4f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(
                animation = tween(800),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "pulse_alpha",
        )
    } else {
        remember { mutableFloatStateOf(1f) }
    }

    TooltipBox(
        positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(),
        tooltip = { Text(label) },
        state = rememberTooltipState(),
    ) {
        Box(
            modifier = modifier
                .size(8.dp)
                .clip(CircleShape)
                .background(color.copy(alpha = alpha)),
        )
    }
}
