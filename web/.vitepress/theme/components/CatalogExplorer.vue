<script setup lang="ts">
// The explorer: every provider, every operation, and an honest account of what stands in the way.
//
// The headline count is the number of operations that own a defect. It is **not** "N of 25 working":
// `works` is false for all 25 today because the auth seam has not landed in flux, and a site that
// made 20 operations working exactly as designed look broken would be as dishonest as one that hid
// the five real gaps.
//
// So the presentation follows `scope`, which the emitter puts on every issue for this reason:
//
//   catalog  → a banner here, above everything
//   provider → a banner on that provider's card
//   operation → a badge and the reason, on the operation, wherever it appears

import { computed } from 'vue'
import {
  allOperations,
  catalogIssues,
  defectCount,
  type Catalog,
} from '../../../data/catalog.mts'
import IssueNotice from './IssueNotice.vue'
import OperationList from './OperationList.vue'
import ProviderCard from './ProviderCard.vue'

const props = defineProps<{ catalog: Catalog }>()

const operations = computed(() => allOperations(props.catalog))
const defects = computed(() => defectCount(operations.value))
const wide = computed(() => catalogIssues(props.catalog))
</script>

<template>
  <p class="summary" :data-defect-count="defects">
    <strong>{{ catalog.providers.length }}</strong> connectors ·
    <strong>{{ operations.length }}</strong> operations ·
    <strong>{{ defects }}</strong> with an operation-specific limitation.
    Choose a connector below or filter the complete operation list.
  </p>

  <IssueNotice
    title="Catalogue-wide availability limitation"
    tone="inherited"
    banner="catalog"
    :issues="wide"
  />

  <h2 id="providers">Connectors</h2>
  <div class="providers">
    <ProviderCard
      v-for="provider in catalog.providers"
      :key="provider.id"
      :provider="provider"
    />
  </div>

  <h2 id="operations">Operations</h2>
  <OperationList :providers="catalog.providers" />
</template>

<style scoped>
.summary {
  font-size: 14px;
  color: var(--vp-c-text-2);
}

/*
 * The 320px minimum is kept, and kept on purpose (C-100). `auto-fit` fits
 * `floor((width + gap) / (min + gap))` tracks, so on the 1025px the page now gets this is three
 * columns, and a fourth would need a minimum of 244px or less.
 *
 * 244px is below what these cards can render. A card is `min` wide less 40px of padding, and the
 * widest header — vendor name, id and status badge on one unwrapped flex line — has a min-content
 * width of 274px, so a card needs 314px before the badge escapes its border; twelve of the sixteen
 * do at 244px. Four columns is therefore a card change and not a grid change, and the card belongs
 * to C-103. 320px is the smallest round number above that 314px floor.
 */
.providers {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: 16px;
  margin: 16px 0;
}
</style>
