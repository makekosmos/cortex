package com.kazui.delphi.ui.screens.settings

import android.graphics.Bitmap
import android.graphics.Color
import androidx.camera.core.ExperimentalGetImage
import androidx.compose.foundation.Image
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.kazui.delphi.data.sync.LanSyncState

@OptIn(ExperimentalMaterial3Api::class, ExperimentalGetImage::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val activeSpaceCode by viewModel.activeSpaceCode.collectAsStateWithLifecycle()
    val lanSyncState by viewModel.lanSyncState.collectAsStateWithLifecycle()
    val connectedPeerCount by viewModel.connectedPeerCount.collectAsStateWithLifecycle()
    val connectedPeerNames by viewModel.connectedPeerNames.collectAsStateWithLifecycle()
    var showQr by remember { mutableStateOf(false) }
    var showLegacySync by remember { mutableStateOf(false) }

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
            // ПРОСТРАНСТВО section
            Text(
                "ПРОСТРАНСТВО",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (activeSpaceCode != null) {
                        // Space code (XXXX-XXXX-XXXX) — full width so it doesn't truncate
                        Text(
                            text = viewModel.formatSpaceCode(activeSpaceCode ?: ""),
                            style = MaterialTheme.typography.headlineSmall,
                            fontFamily = FontFamily.Monospace,
                            color = MaterialTheme.colorScheme.primary,
                            maxLines = 1,
                            modifier = Modifier.fillMaxWidth(),
                        )

                        // Connection status
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            val statusColor = when (lanSyncState) {
                                LanSyncState.LIVE -> MaterialTheme.colorScheme.primary
                                LanSyncState.CONNECTED, LanSyncState.SYNCING -> MaterialTheme.colorScheme.tertiary
                                LanSyncState.CONNECTING -> MaterialTheme.colorScheme.secondary
                                LanSyncState.DISCONNECTED -> MaterialTheme.colorScheme.onSurfaceVariant
                            }
                            androidx.compose.foundation.Canvas(
                                modifier = Modifier.size(8.dp),
                            ) {
                                drawCircle(color = statusColor)
                            }
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = when {
                                    connectedPeerCount > 0 -> "$connectedPeerCount ${if (connectedPeerCount == 1) "пир" else "пиров"}"
                                    lanSyncState == LanSyncState.CONNECTING -> "Подключение..."
                                    else -> "Нет подключений"
                                },
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }

                        // Connected peer names
                        if (connectedPeerNames.isNotEmpty()) {
                            for (name in connectedPeerNames) {
                                Text(
                                    text = name,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }

                        // Toggle QR
                        OutlinedButton(
                            onClick = { showQr = !showQr },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text(if (showQr) "Скрыть QR" else "Показать QR для подключения")
                        }

                        // QR code for sharing
                        if (showQr) {
                            val qrPayload = viewModel.getQrPayload()
                            val qrBitmap = remember(qrPayload) { generateSettingsQrBitmap(qrPayload) }
                            if (qrBitmap != null) {
                                Image(
                                    bitmap = qrBitmap.asImageBitmap(),
                                    contentDescription = "QR-код пространства",
                                    modifier = Modifier
                                        .size(200.dp)
                                        .align(Alignment.CenterHorizontally),
                                )
                            }
                        }

                        // "Покинуть пространство" button
                        OutlinedButton(
                            onClick = { viewModel.leaveSpace() },
                            modifier = Modifier.fillMaxWidth(),
                            colors = ButtonDefaults.outlinedButtonColors(
                                contentColor = MaterialTheme.colorScheme.error,
                            ),
                        ) {
                            Text("Покинуть пространство")
                        }
                    } else {
                        Text(
                            "Пространство не настроено",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            Spacer(Modifier.weight(1f))
            Text(
                "Версия 1.0.0",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .align(Alignment.CenterHorizontally)
                    .combinedClickable(
                        onClick = {},
                        onLongClick = { showLegacySync = !showLegacySync },
                    ),
            )
        }
    }
}

private fun generateSettingsQrBitmap(data: String, size: Int = 512): Bitmap? {
    return try {
        val writer = com.google.zxing.qrcode.QRCodeWriter()
        val bitMatrix = writer.encode(data, com.google.zxing.BarcodeFormat.QR_CODE, size, size)
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        for (x in 0 until size) {
            for (y in 0 until size) {
                bitmap.setPixel(x, y, if (bitMatrix.get(x, y)) Color.BLACK else Color.WHITE)
            }
        }
        bitmap
    } catch (e: Exception) {
        null
    }
}
