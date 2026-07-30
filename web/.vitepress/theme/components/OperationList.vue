<script setup lang="ts">
// The filterable operation list.
//
// Every option in every select is read out of the catalogue — add a provider, a risk tier or an
// idempotency class and the filters grow to cover it with no edit here.
//
// The last filter is the one that earns its place, and it is deliberately **not** "does it work".
// `works` is false for all 25 operations today because no provider can make a live call yet, so
// filtering on it sorts nothing from nothing. Filtering on whether an operation owns a defect
// separates the five with a problem of their own from the twenty waiting on the same seam as
// everything else.
//
// Filtering is plain component state. It narrows a list that is already fully rendered, so with
// JavaScript switched off every operation is still on the page.

import { computed, ref } from 'vue'
import { facet, ownsDefect, type Operation, type Provider } from '../../../data/catalog.mts'
import OperationRow from './OperationRow.vue'

const props = defineProps<{ providers: Provider[] }>()

const ANY = ''

const query = ref('')
const provider = ref(ANY)
const risk = ref(ANY)
const idempotency = ref(ANY)
const defect = ref(ANY)

/** Every operation with the vendor that owns it, flattened in catalogue order. */
const entries = computed(() =>
  props.providers.flatMap((owner) =>
    owner.operations.map((operation) => ({ operation, owner }))
  )
)

const operations = computed<Operation[]>(() => entries.value.map((entry) => entry.operation))

const risks = computed(() => facet(operations.value, (operation) => operation.risk))
const idempotencies = computed(() =>
  facet(operations.value, (operation) => operation.idempotency)
)

const shown = computed(() =>
  entries.value.filter(({ operation, owner }) => {
    if (provider.value !== ANY && owner.id !== provider.value) return false
    if (risk.value !== ANY && operation.risk !== risk.value) return false
    if (idempotency.value !== ANY && operation.idempotency !== idempotency.value) return false
    if (defect.value === 'own' && !ownsDefect(operation)) return false
    if (defect.value === 'none' && ownsDefect(operation)) return false

    const needle = query.value.trim().toLowerCase()
    if (!needle) return true
    return (
      operation.id.toLowerCase().includes(needle) ||
      operation.description.toLowerCase().includes(needle) ||
      operation.path.toLowerCase().includes(needle)
    )
  })
)

function reset() {
  query.value = ''
  provider.value = ANY
  risk.value = ANY
  idempotency.value = ANY
  defect.value = ANY
}
</script>

<template>
  <div class="filters">
    <label class="filters__field filters__field--wide">
      <span>Search</span>
      <input v-model="query" type="search" placeholder="id, description or path" />
    </label>

    <label class="filters__field">
      <span>Provider</span>
      <select v-model="provider">
        <option :value="ANY">Any</option>
        <option v-for="owner in providers" :key="owner.id" :value="owner.id">
          {{ owner.vendor }}
        </option>
      </select>
    </label>

    <label class="filters__field">
      <span>Risk</span>
      <select v-model="risk">
        <option :value="ANY">Any</option>
        <option v-for="value in risks" :key="value" :value="value">{{ value }}</option>
      </select>
    </label>

    <label class="filters__field">
      <span>Idempotency</span>
      <select v-model="idempotency">
        <option :value="ANY">Any</option>
        <option v-for="value in idempotencies" :key="value" :value="value">{{ value }}</option>
      </select>
    </label>

    <label class="filters__field">
      <span>Defects</span>
      <select v-model="defect">
        <option :value="ANY">Any</option>
        <option value="own">Owns a known defect</option>
        <option value="none">No known defect</option>
      </select>
    </label>

    <button class="filters__reset" type="button" @click="reset">Reset</button>
  </div>

  <p class="count">
    Showing <strong>{{ shown.length }}</strong> of {{ operations.length }} operations.
  </p>

  <ul class="list">
    <OperationRow
      v-for="{ operation, owner } in shown"
      :key="operation.id"
      :operation="operation"
      :vendor="owner.vendor"
    />
  </ul>

  <p v-if="!shown.length" class="count">Nothing matches those filters.</p>
</template>

<style scoped>
.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 12px;
  margin: 16px 0;
}

.filters__field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.filters__field--wide {
  flex: 1 1 220px;
}

.filters__field input,
.filters__field select {
  border: 1px solid var(--vp-c-divider);
  border-radius: 6px;
  padding: 5px 8px;
  font-size: 14px;
  color: var(--vp-c-text-1);
  background-color: var(--vp-c-bg);
  width: 100%;
}

.filters__reset {
  border: 1px solid var(--vp-c-divider);
  border-radius: 6px;
  padding: 6px 12px;
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.count {
  font-size: 13px;
  color: var(--vp-c-text-2);
  margin: 8px 0;
}

.list {
  display: grid;
  gap: 12px;
  padding: 0;
  margin: 0;
  list-style: none;
}
</style>
