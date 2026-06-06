package com.kosmos.ark.data.model

import kotlinx.serialization.Serializable

@Serializable
data class RecurrenceData(
    val frequency: Frequency,
    val interval: Int = 1,
    val recurrenceType: RecurrenceType = RecurrenceType.FIXED,
    val daysOfWeek: List<Int>? = null,
    val endDate: String? = null,
)
