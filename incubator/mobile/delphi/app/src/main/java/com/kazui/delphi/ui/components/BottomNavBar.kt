package com.kazui.delphi.ui.components

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CalendarMonth
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.automirrored.filled.LibraryBooks
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.WbSunny
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.vector.ImageVector
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.domain.model.SmartListCounts

data class BottomNavItem(
    val route: String,
    val label: String,
    val icon: ImageVector,
    val smartList: SmartList? = null,
)

val bottomNavItems = listOf(
    BottomNavItem("today", "Сегодня", Icons.Filled.WbSunny, SmartList.TODAY),
    BottomNavItem("inbox", "Входящие", Icons.Filled.Inbox, SmartList.INBOX),
    BottomNavItem("upcoming", "Планы", Icons.Filled.CalendarMonth, SmartList.UPCOMING),
    BottomNavItem("logbook", "Журнал", Icons.AutoMirrored.Filled.LibraryBooks, SmartList.LOGBOOK),
    BottomNavItem("settings", "Настройки", Icons.Filled.Settings),
)

@Composable
fun BottomNavBar(
    currentRoute: String,
    counts: SmartListCounts,
    onNavigate: (String) -> Unit,
) {
    NavigationBar(
        containerColor = MaterialTheme.colorScheme.surface,
    ) {
        bottomNavItems.forEach { item ->
            val selected = currentRoute == item.route
            val badgeCount = item.smartList?.let { counts.get(it) } ?: 0

            NavigationBarItem(
                selected = selected,
                onClick = { onNavigate(item.route) },
                icon = {
                    if (badgeCount > 0) {
                        BadgedBox(badge = {
                            Badge { Text(badgeCount.toString()) }
                        }) {
                            Icon(item.icon, contentDescription = item.label)
                        }
                    } else {
                        Icon(item.icon, contentDescription = item.label)
                    }
                },
                label = { Text(item.label) },
            )
        }
    }
}
