<script setup lang="ts">
// SqobaPage — stub.
//
// SQOBA (save-game backup) — это файловые операции на диске:
// чтение saved-games manifest, copy / restore save files, compression.
// Всё требует main-process IPC, которого extension renderer не имеет.
// Phase 5 решит, где хост этих операций.
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">SQOBA</h1>
    <p class="arrancador-page__hint">
      Резервное копирование сохранений требует доступа к диску и работает
      пока только в legacy Arrancador.exe. Раздел появится здесь после
      Phase 5 (file IO через kepler-backend extension).
    </p>
    <div class="arrancador-empty">
      <p class="arrancador-empty__title">Раздел недоступен</p>
      <p class="arrancador-empty__hint">
        Откройте Arrancador.exe → SQOBA для управления бэкапами сохранений.
      </p>
    </div>
  </section>
</template>
