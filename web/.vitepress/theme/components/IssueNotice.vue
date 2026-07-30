<script setup lang="ts">
// One block of reasons something does not work, rendered from the catalogue's own text.
//
// Used at all three scopes. The tone is the only thing that differs, and it is deliberate: a defect
// the operation owns is a warning about *this* operation, while a catalogue- or provider-wide
// condition is context — stated in full, once, rather than repeated as 25 individual failures.

import type { Issue } from '../../../data/catalog.mts'

defineProps<{
  title: string
  issues: Issue[]
  tone: 'defect' | 'inherited'
  /** Marks this block as a banner over a whole set rather than a note about one operation. */
  banner?: 'catalog' | 'provider'
  /** The provider a banner covers, when it covers one. */
  provider?: string
}>()
</script>

<template>
  <section
    v-if="issues.length"
    class="notice"
    :class="`notice--${tone}`"
    :data-banner="banner"
    :data-provider="provider"
  >
    <h4 class="notice__title">{{ title }}</h4>
    <ul class="notice__list">
      <li v-for="issue in issues" :key="issue.code" class="notice__item">
        <p class="notice__summary">{{ issue.summary }}</p>
        <p class="notice__meta">
          <span class="notice__code">{{ issue.code }}</span>
          <span class="notice__story">closed by {{ issue.story }}</span>
          <span v-if="issue.params.length" class="notice__params">
            affects
            <code v-for="name in issue.params" :key="name">{{ name }}</code>
          </span>
        </p>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.notice {
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 12px 16px;
  margin: 16px 0;
}

.notice--defect {
  background-color: var(--vp-c-danger-soft);
  border-color: var(--vp-c-danger-1);
}

.notice--inherited {
  background-color: var(--vp-c-default-soft);
  border-color: var(--vp-c-divider);
}

.notice__title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.02em;
}

.notice--defect .notice__title {
  color: var(--vp-c-danger-1);
}

.notice__list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.notice__item + .notice__item {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--vp-c-divider);
}

.notice__summary {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
}

.notice__meta {
  margin: 6px 0 0;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.notice__code {
  font-family: var(--vp-font-family-mono);
}

.notice__params code {
  font-size: 11px;
}
</style>
