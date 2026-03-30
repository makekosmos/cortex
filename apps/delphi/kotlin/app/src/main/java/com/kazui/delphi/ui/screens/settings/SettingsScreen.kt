package com.kazui.delphi.ui.screens.settings

import androidx.camera.core.ExperimentalGetImage
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.kazui.delphi.data.sync.SyncStatus
import com.kazui.delphi.ui.components.ConnectionIndicator
import com.kazui.delphi.ui.components.QRScannerScreen

@OptIn(ExperimentalMaterial3Api::class, ExperimentalGetImage::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val syncStatus by viewModel.syncStatus.collectAsStateWithLifecycle()
    val savedUrl by viewModel.savedUrl.collectAsStateWithLifecycle()
    var inputText by remember { mutableStateOf("") }
    var showQr by remember { mutableStateOf(false) }

    if (showQr) {
        QRScannerScreen(
            onScan = { data ->
                showQr = false
                viewModel.connect(data)
            },
            onClose = { showQr = false },
        )
        return
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Настройки") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                "СИНХРОНИЗАЦИЯ ARK",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (savedUrl != null) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            ConnectionIndicator(status = syncStatus)
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = when (syncStatus) {
                                    SyncStatus.ONLINE -> "Подключено"
                                    SyncStatus.SYNCING -> "Подключение..."
                                    SyncStatus.OFFLINE -> "Не подключено"
                                },
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                        Text(
                            text = savedUrl ?: "",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { viewModel.resetSync() },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Переcинхронизировать")
                        }
                        OutlinedButton(
                            onClick = { viewModel.disconnect() },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Отключить")
                        }
                    } else {
                        OutlinedTextField(
                            value = inputText,
                            onValueChange = { inputText = it },
                            placeholder = { Text("ark://host:port?key=...") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedButton(
                                onClick = { showQr = true },
                                modifier = Modifier.weight(1f),
                            ) {
                                Icon(
                                    Icons.Default.QrCodeScanner,
                                    contentDescription = null,
                                    modifier = Modifier.size(18.dp),
                                )
                                Spacer(Modifier.width(4.dp))
                                Text("QR")
                            }
                            Button(
                                onClick = { viewModel.connect(inputText) },
                                enabled = inputText.isNotBlank(),
                                modifier = Modifier.weight(1f),
                            ) {
                                Text("Подключить")
                            }
                        }
                    }
                }
            }

            OutlinedButton(
                onClick = { viewModel.clearLocalData() },
                modifier = Modifier.fillMaxWidth(),
                colors = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
            ) {
                Text("Очистить данные")
            }

            Spacer(Modifier.weight(1f))
            Text(
                "Версия 1.0.0",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
        }
    }
}
