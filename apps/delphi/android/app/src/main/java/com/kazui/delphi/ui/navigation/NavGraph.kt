package com.kazui.delphi.ui.navigation

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CalendarMonth
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.WbSunny
import androidx.compose.material.icons.outlined.CalendarMonth
import androidx.compose.material.icons.outlined.GridView
import androidx.compose.material.icons.outlined.Inbox
import androidx.compose.material.icons.outlined.WbSunny
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.kazui.delphi.R
import com.kazui.delphi.ui.screens.inbox.InboxScreen
import com.kazui.delphi.ui.screens.logbook.LogbookScreen
import com.kazui.delphi.ui.screens.more.MoreScreen
import com.kazui.delphi.ui.screens.project.ProjectScreen
import com.kazui.delphi.ui.screens.settings.SettingsScreen
import com.kazui.delphi.ui.screens.today.TodayScreen
import com.kazui.delphi.ui.screens.trash.TrashScreen
import com.kazui.delphi.ui.screens.upcoming.UpcomingScreen

sealed class Screen(
    val route: String,
    val labelRes: Int,
    val selectedIcon: ImageVector,
    val unselectedIcon: ImageVector,
) {
    data object Today : Screen(
        "today", R.string.tab_today,
        Icons.Filled.WbSunny, Icons.Outlined.WbSunny,
    )
    data object Inbox : Screen(
        "inbox", R.string.tab_inbox,
        Icons.Filled.Inbox, Icons.Outlined.Inbox,
    )
    data object Upcoming : Screen(
        "upcoming", R.string.tab_upcoming,
        Icons.Filled.CalendarMonth, Icons.Outlined.CalendarMonth,
    )
    data object More : Screen(
        "more", R.string.tab_more,
        Icons.Filled.GridView, Icons.Outlined.GridView,
    )
}

private val tabScreens = listOf(Screen.Today, Screen.Inbox, Screen.Upcoming, Screen.More)

@Composable
fun DelphiNavGraph() {
    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination
    val showBottomBar = tabScreens.any { it.route == currentDestination?.route }

    Scaffold(
        contentWindowInsets = WindowInsets(0),
        containerColor = MaterialTheme.colorScheme.background,
        bottomBar = {
            if (showBottomBar) {
                NavigationBar(
                    containerColor = MaterialTheme.colorScheme.surface,
                    tonalElevation = 3.dp,
                ) {
                    tabScreens.forEach { screen ->
                        val selected = currentDestination?.hierarchy?.any { it.route == screen.route } == true
                        NavigationBarItem(
                            icon = {
                                Icon(
                                    if (selected) screen.selectedIcon else screen.unselectedIcon,
                                    contentDescription = null,
                                )
                            },
                            label = { Text(stringResource(screen.labelRes)) },
                            selected = selected,
                            onClick = {
                                navController.navigate(screen.route) {
                                    popUpTo(navController.graph.findStartDestination().id) { saveState = true }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            },
                            colors = NavigationBarItemDefaults.colors(
                                selectedIconColor = MaterialTheme.colorScheme.primary,
                                selectedTextColor = MaterialTheme.colorScheme.primary,
                                unselectedIconColor = MaterialTheme.colorScheme.onSurfaceVariant,
                                unselectedTextColor = MaterialTheme.colorScheme.onSurfaceVariant,
                                indicatorColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.12f),
                            ),
                        )
                    }
                }
            }
        },
    ) { paddingValues ->
        NavHost(
            navController = navController,
            startDestination = Screen.Today.route,
            modifier = Modifier.padding(paddingValues),
            enterTransition = { EnterTransition.None },
            exitTransition = { ExitTransition.None },
            popEnterTransition = { EnterTransition.None },
            popExitTransition = { ExitTransition.None },
        ) {
            composable(Screen.Today.route) {
                TodayScreen(onOpenSettings = { navController.navigate("settings") })
            }
            composable(Screen.Inbox.route) {
                InboxScreen(onOpenSettings = { navController.navigate("settings") })
            }
            composable(Screen.Upcoming.route) {
                UpcomingScreen(onOpenSettings = { navController.navigate("settings") })
            }
            composable(Screen.More.route) {
                MoreScreen(
                    onOpenSettings = { navController.navigate("settings") },
                    onNavigateToLogbook = { navController.navigate("logbook") },
                    onNavigateToTrash = { navController.navigate("trash") },
                    onNavigateToProject = { id -> navController.navigate("project/$id") },
                )
            }
            composable("logbook") {
                LogbookScreen(onOpenSettings = { navController.popBackStack() })
            }
            composable("trash") {
                TrashScreen(onBack = { navController.popBackStack() })
            }
            composable("project/{projectId}") {
                ProjectScreen(onBack = { navController.popBackStack() })
            }
            composable(
                route = "settings",
                enterTransition = { slideInVertically(initialOffsetY = { it }) },
                exitTransition = { ExitTransition.None },
                popEnterTransition = { EnterTransition.None },
                popExitTransition = { slideOutVertically(targetOffsetY = { it }) },
            ) {
                SettingsScreen(onBack = { navController.popBackStack() })
            }
        }
    }
}
