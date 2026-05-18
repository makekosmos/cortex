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
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="currentColor"
          aria-hidden="true"
        >
          <path d="M0 3.449L9.75 2.1v9.451H0V3.449zm10.949-1.531L24 0v11.4H10.949V1.918zM0 12.6h9.75v9.451L0 20.699V12.6zm10.949 0H24V24l-12.9-1.818-.151-9.582z" />
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
  font-size: clamp(36px, 5.5vw, 56px);
  font-weight: 700;
  letter-spacing: -0.025em;
  line-height: 1.05;
  color: var(--foreground);
}

.subtitle {
  margin: 16px 0 0;
  font-size: clamp(15px, 1.6vw, 18px);
  line-height: 1.5;
  color: var(--muted);
  font-weight: 400;
}

.cta-row {
  margin-top: 40px;
  display: inline-flex;
  align-items: stretch;
  gap: 0;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 4px;
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 4px 16px rgba(0, 0, 0, 0.4);
}

.cta {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding: 12px 20px;
  background: var(--accent-bright);
  color: white;
  font-weight: 600;
  font-size: 14px;
  border-radius: 999px;
  cursor: pointer;
  border: none;
  transition: background 120ms ease, transform 120ms ease;
  white-space: nowrap;
}
.cta:hover {
  background: #4f8cff;
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
  padding: 6px 20px;
  text-align: left;
  white-space: nowrap;
}

.requirement-label {
  font-size: 11px;
  color: var(--muted-2);
  font-weight: 500;
  letter-spacing: 0.02em;
}

.requirement-value {
  font-size: 13px;
  color: var(--foreground);
  font-weight: 500;
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
