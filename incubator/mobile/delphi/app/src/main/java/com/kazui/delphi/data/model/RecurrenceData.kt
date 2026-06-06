package com.kazui.delphi.data.model

import androidx.compose.runtime.Immutable
import kotlinx.serialization.Serializable

@Immutable
@Serializable
data class RecurrenceData(
    val frequency: Frequency,
    val interval: Int = 1,
    val recurrenceType: RecurrenceType = RecurrenceType.FIXED,
    val daysOfWeek: List<Int>? = null,
    val endDate: String? = null,
)
