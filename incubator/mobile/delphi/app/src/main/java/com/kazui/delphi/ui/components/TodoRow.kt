package com.kazui.delphi.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.kazui.delphi.data.model.Priority
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.ui.theme.AccentBlue
import com.kazui.delphi.ui.theme.AccentRed
import com.kazui.delphi.ui.theme.DarkBackground
import com.kazui.delphi.ui.theme.PriorityHigh
import com.kazui.delphi.ui.theme.PriorityLow
import com.kazui.delphi.ui.theme.PriorityMedium
import com.kazui.delphi.ui.theme.PriorityNone
import com.kazui.delphi.ui.theme.TextPrimary
import com.kazui.delphi.ui.theme.TextSecondary

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TodoRow(
    todo: TodoItem,
    onToggleCompleted: () -> Unit,
    onClick: () -> Unit,
    onDelete: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val borderColor: Color = when (todo.priority) {
        Priority.HIGH -> PriorityHigh
        Priority.MEDIUM -> PriorityMedium
        Priority.LOW -> PriorityLow
        Priority.NONE -> PriorityNone
    }

    if (onDelete != null) {
        val dismissState = rememberSwipeToDismissBoxState(
            confirmValueChange = { value ->
                when (value) {
                    SwipeToDismissBoxValue.StartToEnd -> {
                        onToggleCompleted()
                        false // don't dismiss, just toggle
                    }
                    SwipeToDismissBoxValue.EndToStart -> {
                        onDelete()
                        true
                    }
                    SwipeToDismissBoxValue.Settled -> false
                }
            },
        )

        SwipeToDismissBox(
            state = dismissState,
            backgroundContent = {
                val direction = dismissState.dismissDirection
                val isStart = direction == SwipeToDismissBoxValue.StartToEnd
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(if (isStart) AccentBlue.copy(alpha = 0.3f) else AccentRed.copy(alpha = 0.3f))
                        .padding(horizontal = 24.dp),
                    contentAlignment = if (isStart) Alignment.CenterStart else Alignment.CenterEnd,
                ) {
                    Icon(
                        imageVector = if (isStart) Icons.Default.Check else Icons.Default.Delete,
                        contentDescription = null,
                        tint = if (isStart) AccentBlue else AccentRed,
                    )
                }
            },
            modifier = modifier,
        ) {
            TodoRowContent(todo, onToggleCompleted, onClick, borderColor)
        }
    } else {
        TodoRowContent(todo, onToggleCompleted, onClick, borderColor, modifier)
    }
}

@Composable
private fun TodoRowContent(
    todo: TodoItem,
    onToggleCompleted: () -> Unit,
    onClick: () -> Unit,
    borderColor: Color,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.background)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Checkbox circle
        if (todo.isCompleted) {
            Box(
                modifier = Modifier
                    .size(26.dp)
                    .clip(CircleShape)
                    .background(AccentBlue)
                    .clickable(onClick = onToggleCompleted),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Default.Check,
                    contentDescription = null,
                    tint = Color.White,
                    modifier = Modifier.size(16.dp),
                )
            }
        } else {
            Box(
                modifier = Modifier
                    .size(26.dp)
                    .clip(CircleShape)
                    .border(2.dp, borderColor, CircleShape)
                    .clickable(onClick = onToggleCompleted),
            )
        }

        Spacer(modifier = Modifier.width(14.dp))

        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = todo.title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (todo.isCompleted) TextSecondary else TextPrimary,
                textDecoration = if (todo.isCompleted) TextDecoration.LineThrough else TextDecoration.None,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )

            if (!todo.notes.isNullOrBlank()) {
                Text(
                    text = todo.notes,
                    style = MaterialTheme.typography.bodyMedium,
                    color = TextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }

            val hasDate = todo.scheduledDate != null || todo.deadline != null
            if (hasDate) {
                val scheduledFormatted = remember(todo.scheduledDate) { todo.scheduledDate?.let { d -> formatDate(d) } }
                val deadlineFormatted = remember(todo.deadline) { todo.deadline?.let { d -> formatDate(d) } }
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(top = 4.dp),
                ) {
                    scheduledFormatted?.let {
                        Text(
                            text = it,
                            style = MaterialTheme.typography.labelMedium,
                            color = AccentBlue,
                        )
                    }
                    deadlineFormatted?.let {
                        Text(
                            text = it,
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
