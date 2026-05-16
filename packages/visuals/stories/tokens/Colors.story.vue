<script setup lang="ts">
import { colors } from "../../tokens";

type Mode = "light" | "dark";

const semanticTokens = Object.keys(colors.light) as (keyof typeof colors.light)[];
const statusTokens = Object.keys(colors.status) as (keyof typeof colors.status)[];
const smartListTokens = Object.keys(colors.smartList) as (keyof typeof colors.smartList)[];

function readableLabel(token: string): string {
  return token.replace(/([A-Z])/g, " $1").trim();
}

function swatchValue(mode: Mode, token: keyof typeof colors.light): string {
  return colors[mode][token];
}
</script>

<template>
  <Story title="Colors" group="tokens" :layout="{ type: 'single', iframe: false }">
    <Variant title="Semantic (light vs dark)">
      <div class="story-canvas">
        <p class="story-label">Семантические цвета — параллельные палитры для light/dark</p>
        <table class="palette">
          <thead>
            <tr>
              <th>token</th>
              <th>light</th>
              <th>dark</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="t in semanticTokens" :key="t">
              <td>
                <code>{{ readableLabel(t) }}</code>
              </td>
              <td>
                <span class="chip" :style="{ background: swatchValue('light', t) }" />
                <code>{{ swatchValue("light", t) }}</code>
              </td>
              <td>
                <span class="chip" :style="{ background: swatchValue('dark', t) }" />
                <code>{{ swatchValue("dark", t) }}</code>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </Variant>

    <Variant title="Status">
      <div class="story-canvas">
        <p class="story-label">Утилитарные тоны статуса</p>
        <div class="story-row">
          <div v-for="t in statusTokens" :key="t" class="status-card">
            <span class="chip chip--lg" :style="{ background: colors.status[t] }" />
            <code>{{ t }}</code>
            <code class="muted">{{ colors.status[t] }}</code>
          </div>
        </div>
      </div>
    </Variant>

    <Variant title="Smart-list (Delphi)">
      <div class="story-canvas">
        <p class="story-label">Цвета smart-list’ов в Delphi (Today, Inbox, Logbook…)</p>
        <div class="story-row">
          <div v-for="t in smartListTokens" :key="t" class="status-card">
            <span class="chip chip--lg" :style="{ background: colors.smartList[t] }" />
            <code>{{ t }}</code>
            <code class="muted">{{ colors.smartList[t] }}</code>
          </div>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.palette {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.825rem;
}
.palette th,
.palette td {
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--border);
  text-align: left;
  vertical-align: middle;
}
.palette th {
  font-weight: 600;
  text-transform: uppercase;
  font-size: 0.7rem;
  color: var(--muted-foreground);
}
.chip {
  display: inline-block;
  width: 18px;
  height: 18px;
  border-radius: 6px;
  border: 1px solid var(--border);
  vertical-align: middle;
  margin-right: 0.5rem;
}
.chip--lg {
  width: 56px;
  height: 56px;
  border-radius: 12px;
}
.status-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.35rem;
}
code {
  font-family: var(--font-mono);
  font-size: 0.75rem;
}
.muted {
  color: var(--muted-foreground);
}
</style>
