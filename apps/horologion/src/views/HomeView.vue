<script setup lang="ts">
import PomodoroView from "./PomodoroView.vue";
import ListView from "./ListView.vue";
</script>

<template>
    <div class="home">
        <!-- Верхний прямоугольник — pomodoro как фокусная зона страницы. -->
        <section class="home__pomo">
            <PomodoroView />
        </section>

        <!-- Нижняя часть — список записей дня/недели. -->
        <section class="home__list">
            <ListView />
        </section>
    </div>
</template>

<style scoped>
.home {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1rem 0;
}

.home__pomo {
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 1.75rem 1rem;
    background: color-mix(in srgb, var(--foreground) 4%, var(--background));
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) * 3);
    corner-shape: var(--corner-shape);
}
</style>
