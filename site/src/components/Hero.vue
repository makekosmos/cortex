<script setup lang="ts">
const DOWNLOAD_URL =
    "https://github.com/yoso-industries/kepler-releases/releases/latest";
</script>

<template>
    <section class="hero">
        <h1 class="title">Твой личный Kosmos</h1>
        <p class="subtitle">
            Local-first приложение с нативными агентами,<br />
            помогающее сохранить все самое важное.
        </p>

        <div class="cta-row">
            <a :href="DOWNLOAD_URL" class="cta" target="_blank" rel="noopener">
                <svg
                    class="cta-icon"
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    fill="none"
                    viewBox="0 0 16 16"
                    aria-hidden="true"
                >
                    <path
                        fill="currentColor"
                        fill-rule="evenodd"
                        d="M0 0h7.584v7.584H0zm8.416 0h7.583v7.584H8.416zm-.832 8.416H0V16h7.584zm.832 0h7.583V16H8.416z"
                        clip-rule="evenodd"
                    />
                </svg>
                <span>Скачать для Windows (Beta)</span>
            </a>

            <div class="requirement">
                <div class="requirement-label">Требование</div>
                <div class="requirement-value">Не ниже Windows 10 1809</div>
            </div>
        </div>
    </section>
</template>

<style scoped>
.hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 640px;
}

.title {
    margin: 0;
    font-size: 48px;
    font-weight: 600;
    letter-spacing: -0.025em;
    line-height: 1;
    color: var(--foreground);
}

.subtitle {
    margin: 20px 0 0;
    font-size: 24px;
    line-height: 1.4;
    color: var(--muted);
    font-weight: 400;
}

.cta-row {
    margin-top: 30px;
    display: inline-flex;
    align-items: stretch;
    gap: 0;
    background: #ffffff;
    border-radius: 999px;
    padding: 2px 18px 2px 2px;
    box-shadow:
        0 1px 0 rgba(255, 255, 255, 0.6) inset,
        0 8px 24px rgba(0, 0, 0, 0.35);
}

.cta {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 14px 18px;
    background: #095fff;
    color: white;
    font-weight: 500;
    font-size: 16px;
    border-radius: 999px;
    cursor: pointer;
    border: none;
    transition:
        background 120ms ease,
        transform 120ms ease;
    white-space: nowrap;
}
.cta:hover {
    background: #2a73ff;
}
.cta:active {
    transform: scale(0.98);
}

.cta-icon {
    flex-shrink: 0;
}

.requirement {
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding-left: 10px;
    text-align: left;
    white-space: nowrap;
}

.requirement-label {
    font-size: 14px;
    color: #6b6b6b;
    font-weight: 500;
    letter-spacing: 0.01em;
}

.requirement-value {
    font-size: 14px;
    color: #6b6b6b;
    font-weight: 400;
    margin-top: 2px;
}

@media (max-width: 540px) {
    .cta-row {
        flex-direction: column;
        border-radius: 16px;
        padding: 8px;
        align-items: stretch;
    }
    .cta {
        justify-content: center;
    }
    .requirement {
        padding: 8px 12px 4px;
        text-align: center;
        white-space: normal;
    }
}
</style>
