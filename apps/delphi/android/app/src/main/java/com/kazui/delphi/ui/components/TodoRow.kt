package com.kazui.delphi.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import com.kazui.delphi.data.model.Priority
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.ui.theme.AccentBlue
import com.kazui.delphi.ui.theme.AccentGreen
import com.kazui.delphi.ui.theme.AccentRed
import com.kazui.delphi.ui.theme.DarkBackground
import com.kazui.delphi.ui.theme.PriorityHigh
import com.kazui.delphi.ui.theme.PriorityLow
import com.kazui.delphi.ui.theme.PriorityMedium
import com.kazui.delphi.ui.theme.PriorityNone
import com.kazui.delphi.ui.theme.TextPrimary
import com.kazui.delphi.ui.theme.TextSecondary

@Composable
fun TodoRow(
    todo: TodoItem,
    onToggleCompleted: () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val priorityColor by animateColorAsState(
        targetValue = when (todo.priority) {
            Priority.HIGH -> PriorityHigh
            Priority.MEDIUM -> PriorityMedium
            Priority.LOW -> PriorityLow
            Priority.NONE -> PriorityNone
        },
        label = "priorityColor",
    )

    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(22.dp)
                .clip(CircleShape)
                .clickable(onClick = onToggleCompleted),
            contentAlignment = Alignment.Center,
        ) {
            if (todo.isCompleted) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    shape = CircleShape,
                    color = AccentGreen,
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        Icon(
                            Icons.Default.Check,
                            contentDescription = null,
                            tint = DarkBackground,
                            modifier = Modifier.size(14.dp),
                        )
                    }
                }
            } else {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    shape = CircleShape,
                    color = priorityColor.copy(alpha = 0.2f),
                ) {}
                Surface(
                    modifier = Modifier.size(14.dp),
                    shape = CircleShape,
                    color = priorityColor,
                ) {}
            }
        }

        Spacer(modifier = Modifier.width(12.dp))

        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = todo.title,
                style = MaterialTheme.typography.bodyMedium,
                color = if (todo.isCompleted) TextSecondary else TextPrimary,
                textDecoration = if (todo.isCompleted) TextDecoration.LineThrough else TextDecoration.None,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )

            if (!todo.notes.isNullOrBlank()) {
                Text(
                    text = todo.notes,
                    style = MaterialTheme.typography.bodySmall,
                    color = TextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            val hasDate = todo.scheduledDate != null || todo.deadline != null
            if (hasDate) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(top = 2.dp),
                ) {
                    todo.scheduledDate?.let {
                        Text(
                            text = formatDate(it),
                            style = MaterialTheme.typography.labelMedium,
                            color = AccentBlue,
                        )
                    }
                    todo.deadline?.let {
                        Text(
                            text = formatDate(it),
                            style = MaterialTheme.typography.labelMedium,
                            color = AccentRed,
                        )
                    }
                }
            }
        }
    }
}

private fun formatDate(isoDate: String): String {
    return try {
        val date = java.time.LocalDate.parse(isoDate.take(10))
        val formatter = java.time.format.DateTimeFormatter.ofPattern("d MMM", java.util.Locale.forLanguageTag("ru"))
        date.format(formatter)
    } catch (_: Exception) {
        isoDate.take(10)
    }
}
