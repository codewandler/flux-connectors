<script setup lang="ts">
// The filterable operation list.
//
// Every option in every select is read out of the catalogue — add a provider, a service, a risk tier
// or an idempotency class and the filters grow to cover it with no edit here.
//
// The service filter is the one that depends on another: choosing a connector narrows the services
// to that connector's, because every other connector's service is an option that could not match a
// single row. Choosing a service with no connector chosen stays valid, and is the useful direction —
// it is how a visitor finds one surface of a large vendor without knowing which vendor publishes it.
//
// The last filter is the one that earns its place, and it is deliberately **not** "does it work".
// `works` is false for all 25 operations today because no provider can make a live call yet, so
// filtering on it sorts nothing from nothing. Filtering on whether an operation owns a defect
// separates the five with a problem of their own from the twenty waiting on the same seam as
// everything else.
//
// Filtering is plain component state. It narrows a list that is already fully rendered, so with
// JavaScript switched off every operation is still on the page.

import { computed, ref, watch } from 'vue'
import {
  facet,
  operationService,
  ownsDefect,
  serviceFacet,
  type Operation,
  type Provider,
} from '../../../data/catalog.mts'
import OperationRow from './OperationRow.vue'

const props = defineProps<{ providers: Provider[] }>()

const ANY = ''

const query = ref('')
const provider = ref(ANY)
const service = ref(ANY)
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

/**
 * The service options, narrowed to the chosen connector.
 *
 * The narrowing leaves nothing to choose for a connector that addresses a single surface, so the
 * control is disabled rather than removed: a filter that disappeared under the cursor would move
 * every control beside it.
 */
const services = computed(() => serviceFacet(props.providers, provider.value))

// A chosen service that the chosen connector does not publish would filter every operation away and
// read as an empty catalogue. Narrowing the options narrows the choice with them.
watch(services, (options) => {
  if (service.value !== ANY && !options.includes(service.value)) service.value = ANY
})

const shown = computed(() =>
  entries.value.filter(({ operation, owner }) => {
    if (provider.value !== ANY && owner.id !== provider.value) return false
    if (service.value !== ANY && operation.service !== service.value) return false
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
  service.value = ANY
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
      <span>Connector</span>
      <select v-model="provider">
        <option :value="ANY">Any</option>
        <option v-for="owner in providers" :key="owner.id" :value="owner.id">
          {{ owner.vendor }}
        </option>
      </select>
    </label>

    <label class="filters__field">
      <span>Service</span>
      <select v-model="service" :disabled="!services.length">
        <option :value="ANY">Any</option>
        <option v-for="value in services" :key="value" :value="value">{{ value }}</option>
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
      <span>Operation issues</span>
      <select v-model="defect">
        <option :value="ANY">Any</option>
        <option value="own">Has a known limitation</option>
        <option value="none">No operation-specific issue</option>
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
      :service="operationService(owner, operation)"
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
