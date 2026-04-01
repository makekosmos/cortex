package com.kazui.delphi.ui.screens.space

import android.graphics.Bitmap
import android.graphics.Color
import androidx.camera.core.ExperimentalGetImage
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kazui.delphi.ui.components.QRScannerScreen

/**
 * Displays the connection code with dashes.
 * Handles 7-char (XXXX-XXX), 12-char (XXXX-XXXX-XXXX),
 * and 19-char extended (XXXX-XXXX-XXXX-XXXX-XXX) formats.
 */
class SpaceCodeTransformation : VisualTransformation {
    private val dashPositions = listOf(4, 8, 12, 16)

    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text.uppercase()
        val formatted = buildString {
            raw.forEachIndexed { i, c ->
                if (i in dashPositions && raw.length > i) append('-')
                append(c)
            }
        }
        val offsetMapping = object : OffsetMapping {
            override fun originalToTransformed(offset: Int): Int =
                offset + dashPositions.count { it <= offset && raw.length > it }

            override fun transformedToOriginal(offset: Int): Int {
                var result = offset
                if (offset > 4) result--
                if (offset > 9) result--
                if (offset > 14) result--
                if (offset > 19) result--
                return result.coerceIn(0, raw.length)
            }
        }
        return TransformedText(AnnotatedString(formatted), offsetMapping)
    }
}

private enum class SetupMode { CHOOSE, CREATE, JOIN }

@OptIn(ExperimentalGetImage::class)
@Composable
fun SpaceSetupScreen(
    onSpaceJoined: (String) -> Unit,
    viewModel: SpaceSetupViewModel = hiltViewModel(),
) {
    var mode by remember { mutableStateOf(SetupMode.CHOOSE) }
    var input by remember { mutableStateOf("") }
    var error by remember { mutableStateOf("") }
    var showQr by remember { mutableStateOf(false) }

    // Create mode state
    var generatedCode by remember { mutableStateOf("") }
    var displayCode by remember { mutableStateOf("") } // 19-char extended or 12-char fallback
    var qrPayload by remember { mutableStateOf("") }

    fun sanitize(raw: String): String =
        raw.replace("-", "").replace(" ", "").uppercase()
            .filter { it in "0123456789ABCDEFGHJKMNPQRSTVWXYZ" }
            .take(19) // support 19-char extended codes

    fun handleJoin() {
        val code = input
        // Try 19-char extended code first (has embedded IPv4)
        if (code.length == 19) {
            val parsed = viewModel.parseExtendedCode(code)
            if (parsed != null) {
                val (secret, addresses) = parsed
                error = ""
                viewModel.joinSpace(secret, addresses)
                onSpaceJoined(secret)
                return
            }
        }
        if (!viewModel.isValidCode(code)) {
            error = "Введите корректный код (XXXX-XXXX-XXXX или XXXX-XXXX-XXXX-XXXX-XXX)"
            return
        }
        error = ""
        viewModel.joinSpace(code, emptyList())
        onSpaceJoined(code)
    }

    if (showQr) {
        QRScannerScreen(
            onScan = { data ->
                showQr = false
                // Parse ark://join?code=...&addrs=... QR payload
                val parsed = viewModel.parseQrPayload(data)
                if (parsed != null) {
                    val (code, addresses) = parsed
                    input = code
                    error = ""
                    viewModel.joinSpace(code, addresses)
                    onSpaceJoined(code)
                } else {
                    // Try as raw code
                    val scanned = sanitize(data)
                    if (scanned.length == 12 || scanned.length == 7) {
                        input = scanned
                        error = ""
                    } else {
                        error = "QR-код не содержит корректный код подключения"
                    }
                }
            },
            onClose = { showQr = false },
        )
        return
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp),
            contentAlignment = Alignment.Center,
        ) {
            when (mode) {
                SetupMode.CHOOSE -> {
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                        elevation = CardDefaults.cardElevation(defaultElevation = 4.dp),
                    ) {
                        Column(
                            modifier = Modifier.padding(24.dp),
                            verticalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            Text("Пространство", style = MaterialTheme.typography.titleLarge)
                            Text(
                                "Создайте новое пространство или присоединитесь к существующему.",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Button(
                                onClick = {
                                    generatedCode = viewModel.generateCode()
                                    displayCode = viewModel.generateExtendedCode(generatedCode) ?: generatedCode
                                    qrPayload = viewModel.generateQrPayload(generatedCode)
                                    mode = SetupMode.CREATE
                                },
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Создать пространство")
                            }
                            OutlinedButton(
                                onClick = { mode = SetupMode.JOIN },
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Присоединиться")
                            }
                        }
                    }
                }

                SetupMode.CREATE -> {
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                        elevation = CardDefaults.cardElevation(defaultElevation = 4.dp),
                    ) {
                        Column(
                            modifier = Modifier.padding(24.dp),
                            verticalArrangement = Arrangement.spacedBy(16.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                IconButton(onClick = { mode = SetupMode.CHOOSE }) {
                                    Icon(Icons.AutoMirrored.Filled.ArrowBack, "Назад")
                                }
                                Spacer(Modifier.width(8.dp))
                                Text("Новое пространство", style = MaterialTheme.typography.titleLarge)
                            }

                            // Display extended code (with embedded IP) for manual entry
                            Text(
                                text = viewModel.formatCode(displayCode.ifEmpty { generatedCode }),
                                style = if (displayCode.length > 12)
                                    MaterialTheme.typography.titleMedium
                                else
                                    MaterialTheme.typography.headlineMedium,
                                fontFamily = FontFamily.Monospace,
                                color = MaterialTheme.colorScheme.primary,
                            )

                            Text(
                                "Отсканируйте QR-код или введите длинный код на другом устройстве.",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )

                            // QR code bitmap
                            val qrBitmap = remember(qrPayload) { generateQrBitmap(qrPayload) }
                            if (qrBitmap != null) {
                                Image(
                                    bitmap = qrBitmap.asImageBitmap(),
                                    contentDescription = "QR-код пространства",
                                    modifier = Modifier.size(200.dp),
                                )
                            }

                            Button(
                                onClick = {
                                    viewModel.createSpace(generatedCode)
                                    onSpaceJoined(generatedCode)
                                },
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Активировать")
                            }
                        }
                    }
                }

                SetupMode.JOIN -> {
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                        elevation = CardDefaults.cardElevation(defaultElevation = 4.dp),
                    ) {
                        Column(
                            modifier = Modifier.padding(24.dp),
                            verticalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                IconButton(onClick = { mode = SetupMode.CHOOSE }) {
                                    Icon(Icons.AutoMirrored.Filled.ArrowBack, "Назад")
                                }
                                Spacer(Modifier.width(8.dp))
                                Text("Присоединиться", style = MaterialTheme.typography.titleLarge)
                            }
                            Text(
                                "Отсканируйте QR-код или введите код пространства. Длинный код (XXXX-XXXX-XXXX-XXXX-XXX) подключается напрямую по IP.",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                OutlinedTextField(
                                    value = input,
                                    onValueChange = {
                                        input = sanitize(it)
                                        error = ""
                                    },
                                    placeholder = { Text("XXXX-XXXX-XXXX(-XXXX-XXX)", fontFamily = FontFamily.Monospace) },
                                    label = { Text("Код пространства") },
                                    modifier = Modifier.weight(1f),
                                    singleLine = true,
                                    isError = error.isNotEmpty(),
                                    supportingText = if (error.isNotEmpty()) {
                                        { Text(error, color = MaterialTheme.colorScheme.error) }
                                    } else null,
                                    visualTransformation = SpaceCodeTransformation(),
                                    keyboardOptions = KeyboardOptions(
                                        capitalization = KeyboardCapitalization.Characters,
                                        imeAction = ImeAction.Done,
                                    ),
                                    keyboardActions = KeyboardActions(
                                        onDone = { handleJoin() },
                                    ),
                                )
                                Spacer(Modifier.width(8.dp))
                                IconButton(onClick = { showQr = true }) {
                                    Icon(
                                        Icons.Default.QrCodeScanner,
                                        contentDescription = "Сканировать QR",
                                        modifier = Modifier.size(28.dp),
                                    )
                                }
                            }
                            Button(
                                onClick = { handleJoin() },
                                enabled = input.length == 12 || input.length == 7 || input.length == 19,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Подключиться")
                            }
                        }
                    }
                }
            }
        }
    }
}

/**
 * Generate a simple QR code bitmap using a minimal QR encoder.
 * Uses Android's built-in support: creates bitmap from ZXing-compatible encoding
 * via com.google.mlkit or manual bit matrix.
 *
 * For simplicity, we use a basic approach: encode data into a QR bitmap using
 * a bundled encoder. Since the project already has ML Kit Barcode for scanning,
 * we use a lightweight QR generation approach.
 */
private fun generateQrBitmap(data: String, size: Int = 512): Bitmap? {
    return try {
        // Use ZXing's QRCodeWriter which is bundled with ML Kit Barcode
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
