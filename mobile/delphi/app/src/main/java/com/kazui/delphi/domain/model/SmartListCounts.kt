package com.kazui.delphi.domain.model

import com.kazui.delphi.data.model.SmartList

data class SmartListCounts(val counts: Map<SmartList, Int> = emptyMap()) {
    fun get(list: SmartList): Int = counts[list] ?: 0
}
