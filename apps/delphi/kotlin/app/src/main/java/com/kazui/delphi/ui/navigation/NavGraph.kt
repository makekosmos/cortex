package com.kazui.delphi.ui.navigation

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
import com.kazui.delphi.ui.screens.space.SpaceSetupScreen
import com.kazui.delphi.ui.screens.space.SpaceSetupViewModel
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

private val navTween get() = tween<IntOffset>(durationMillis = 340, easing = FastOutSlowInEasing)

private val slideEnter get() = slideInHorizontally(animationSpec = navTween) { it }
private val slideExit get() = slideOutHorizontally(animationSpec = navTween) { it }

@Composable
fun ArkDataMissingScreen() {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.padding(32.dp),
        ) {
            Text(
                text = "Ark Data не установлен",
                style = MaterialTheme.typography.headlineSmall,
                textAlign = TextAlign.Center,
            )
            Text(
                text = "Приложение ark-data (com.kepler.ark.data) не найдено. " +
                    "Установите его для работы Delphi.",
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
fun DelphiNavGraph(
    spaceViewModel: SpaceSetupViewModel = hiltViewModel(),
) {
    val isInitialized by spaceViewModel.isInitialized.collectAsStateWithLifecycle()
    val activeSpaceCode by spaceViewModel.activeSpaceCode.collectAsStateWithLifecycle()

    // Show nothing while DataStore is loading (avoids flicker)
    if (!isInitialized) return

    // No space configured → show setup screen
    if (activeSpaceCode == null) {
        SpaceSetupScreen(
            onSpaceJoined = { /* DataStore update triggers recomposition */ },
            viewModel = spaceViewModel,
        )
        return
    }

    // ark-data not installed → show error screen
    if (!spaceViewModel.isArkDataAvailable) {
        ArkDataMissingScreen()
        return
    }

    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination
    val showBottomBar = tabScreens.any { it.route == currentDestination?.route }
    @OptIn(ExperimentalLayoutApi::class)
    val isImeVisible = WindowInsets.isImeVisible

    Scaffold(
        contentWindowInsets = WindowInsets(0),
        containerColor = MaterialTheme.colorScheme.background,
        bottomBar = {
            if (showBottomBar && !isImeVisible) {
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
                TodayScreen()
            }
            composable(Screen.Inbox.route) {
                InboxScreen()
            }
            composable(Screen.Upcoming.route) {
                UpcomingScreen()
            }
            composable(Screen.More.route) {
                MoreScreen(
                    onOpenSettings = { navController.navigate("settings") },
                    onNavigateToLogbook = { navController.navigate("logbook") },
                    onNavigateToTrash = { navController.navigate("trash") },
                    onNavigateToProject = { id -> navController.navigate("project/$id") },
                )
            }
            composable(
                route = "logbook",
                enterTransition = { slideEnter },
                exitTransition = { ExitTransition.None },
                popEnterTransition = { EnterTransition.None },
                popExitTransition = { slideExit },
            ) {
                LogbookScreen(onBack = { navController.popBackStack() })
            }
            composable(
                route = "trash",
                enterTransition = { slideEnter },
                exitTransition = { ExitTransition.None },
                popEnterTransition = { EnterTransition.None },
                popExitTransition = { slideExit },
            ) {
                TrashScreen(onBack = { navController.popBackStack() })
            }
            composable(
                route = "project/{projectId}",
                enterTransition = { slideEnter },
                exitTransition = { ExitTransition.None },
                popEnterTransition = { EnterTransition.None },
                popExitTransition = { slideExit },
            ) {
                ProjectScreen(onBack = { navController.popBackStack() })
            }
            composable(
                route = "settings",
                enterTransition = { slideEnter },
                exitTransition = { ExitTransition.None },
                popEnterTransition = { EnterTransition.None },
                popExitTransition = { slideExit },
            ) {
                SettingsScreen(onBack = { navController.popBackStack() })
            }
        }
    }
}
