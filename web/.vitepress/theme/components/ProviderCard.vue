<script setup lang="ts">
// One provider: vendor, operation count, auth scheme, hosts, and a status line that does not
// flatter.
//
// The status line counts the operations that own a defect, not the ones that "work" — see
// `data/catalog.mts`. Whatever holds the whole provider back is stated once, as a banner on the
// card, because repeating it on each of its operations would say nothing about any of them.

import { computed } from 'vue'
import { defectCount, providerIssues, type Provider } from '../../../data/catalog.mts'
import IssueNotice from './IssueNotice.vue'

const props = defineProps<{ provider: Provider }>()

const defects = computed(() => defectCount(props.provider.operations))
const issues = computed(() => providerIssues(props.provider))

/**
 * The provider's headline status, derived so it flips on its own.
 *
 * Today every operation of every provider has `works: false`, so every card reads "not live yet" —
 * which is the truth, and is the badge refusing to flatter. Close the auth seam and these cards
 * change with the next build, with no edit here. What the badge deliberately does not do is count
 * "0 of 9 working": that number is the same for every provider and tells a visitor nothing about
 * which one to pick.
 */
const live = computed(() => props.provider.operations.filter((op) => op.status.works).length)

const headline = computed(() => {
  if (live.value === 0) return { label: 'Not live yet', tone: 'warn' }
  if (live.value < props.provider.operation_count) {
    return { label: `${live.value} operations live`, tone: 'warn' }
  }
  return { label: 'Live', tone: 'ok' }
})
</script>

<template>
  <section :id="provider.id" class="card" :data-provider-defects="defects">
    <header class="card__head">
      <h3 class="card__vendor">{{ provider.vendor }}</h3>
      <code class="card__id">{{ provider.id }}</code>
      <span class="card__badge" :class="`card__badge--${headline.tone}`">
        {{ headline.label }}
      </span>
    </header>

    <p class="card__desc">{{ provider.description }}</p>

    <dl class="card__facts">
      <div>
        <dt>Operations</dt>
        <dd>{{ provider.operation_count }}</dd>
      </div>
      <div>
        <dt>Auth</dt>
        <dd v-if="provider.auth.schemes.length">{{ provider.auth.schemes.join(', ') }}</dd>
        <dd v-else class="card__warn">no credential declared</dd>
      </div>
      <div>
        <dt>Base URL</dt>
        <dd><code>{{ provider.base_url }}</code></dd>
      </div>
      <div>
        <dt>Hosts</dt>
        <dd>
          <code v-for="host in provider.hosts" :key="host">{{ host }}</code>
        </dd>
      </div>
      <div>
        <dt>Own defects</dt>
        <dd :class="defects ? 'card__warn' : 'card__ok'">
          {{ defects }} of {{ provider.operation_count }} operations
        </dd>
      </div>
    </dl>

    <IssueNotice
      title="Affects every operation of this provider"
      tone="inherited"
      banner="provider"
      :provider="provider.id"
      :issues="issues"
    />
  </section>
</template>

<style scoped>
.card {
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 16px 20px;
}

.card__head {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.card__vendor {
  margin: 0;
  font-size: 18px;
  line-height: 1.4;
  border: 0;
  padding: 0;
}

.card__id {
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.card__badge {
  margin-left: auto;
  border-radius: 10px;
  padding: 1px 10px;
  font-size: 12px;
  font-weight: 600;
  line-height: 20px;
  white-space: nowrap;
}

.card__badge--warn {
  background-color: var(--vp-c-warning-soft);
  color: var(--vp-c-warning-1);
}

.card__badge--ok {
  background-color: var(--vp-c-tip-soft);
  color: var(--vp-c-tip-1);
}

.card__desc {
  margin: 6px 0 0;
  font-size: 14px;
  color: var(--vp-c-text-2);
}

.card__facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: 8px 16px;
  margin: 12px 0 0;
}

.card__facts dt {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--vp-c-text-3);
}

.card__facts dd {
  margin: 2px 0 0;
  font-size: 14px;
}

.card__facts code {
  font-size: 12px;
}

.card__warn {
  color: var(--vp-c-danger-1);
  font-weight: 600;
}

.card__ok {
  color: var(--vp-c-text-1);
}
</style>
