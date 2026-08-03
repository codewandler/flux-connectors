<script setup lang="ts">
// One provider: vendor, operation count, auth scheme, hosts, and a status line that does not
// flatter.
//
// The status line counts the operations that own a defect, not the ones that "work" — see
// `data/catalog.mts`. Whatever holds the whole provider back is stated once, as a banner on the
// card, because repeating it on each of its operations would say nothing about any of them.
//
// The services block appears for an explicitly named service or for every surface of a
// multi-surface connector. A connector whose sole surface is the reserved default keeps the compact
// pre-services card; a legacy default beside named siblings is rendered generically as Primary.

import { computed } from 'vue'
import {
  UNPUBLISHED,
  defectCount,
  hasInboundSurface,
  providerAddress,
  providerAuth,
  providerIssues,
  published,
  serviceApiVersion,
  serviceLabel,
  visibleServices,
  type Provider,
} from '../../../data/catalog.mts'
import InboundSurface from './InboundSurface.vue'
import IssueNotice from './IssueNotice.vue'

const props = defineProps<{ provider: Provider }>()

const defects = computed(() => defectCount(props.provider.operations))
const issues = computed(() => providerIssues(props.provider))
const services = computed(() => visibleServices(props.provider))
const address = computed(() => providerAddress(props.provider))

/**
 * The auth this source published, or `null` when it published none (C-408).
 *
 * The three-way branch on the card is the whole of the fix. A connector that publishes auth and
 * lists no scheme is genuinely not configured and still says so in the danger colour; a source that
 * carries no auth at all says only that, in the muted tone a statement about a *document* deserves.
 * Reading `provider.auth.schemes.length` did both at once, and on such a source it threw.
 */
const auth = computed(() => providerAuth(props.provider))

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
        <dd v-if="auth && auth.schemes.length">{{ auth.schemes.join(', ') }}</dd>
        <dd v-else-if="auth" class="card__warn">not configured</dd>
        <dd v-else class="card__unpublished">{{ UNPUBLISHED }}</dd>
      </div>
      <div>
        <dt>Base URL</dt>
        <dd v-if="published(provider.base_url)"><code>{{ provider.base_url }}</code></dd>
        <dd v-else class="card__unpublished">{{ UNPUBLISHED }}</dd>
      </div>
      <div>
        <dt>Hosts</dt>
        <dd class="card__hosts">
          <code v-for="host in provider.hosts" :key="host">{{ host }}</code>
        </dd>
      </div>
      <div v-if="address">
        <dt>Address</dt>
        <dd><code>{{ address }}</code></dd>
      </div>
      <div>
        <dt>Operation-specific issues</dt>
        <dd :class="defects ? 'card__warn' : 'card__ok'">
          {{ defects }} of {{ provider.operation_count }} operations
        </dd>
      </div>
    </dl>

    <div v-if="services.length" class="card__services">
      <h4 class="card__services-title">Services</h4>
      <ul class="services">
        <li
          v-for="service in services"
          :key="service.name"
          class="service"
          :data-service-of="provider.id"
          :data-service="service.name"
        >
          <code class="service__name">{{ serviceLabel(service.name) }}</code>
          <span class="service__count">
            {{ service.operation_count }} operation{{ service.operation_count === 1 ? '' : 's' }}
          </span>
          <span v-if="serviceApiVersion(provider, service)" class="service__version">
            {{ serviceApiVersion(provider, service) }}
          </span>
          <code v-if="service.gid" class="service__gid">{{ service.gid }}</code>
        </li>
      </ul>
    </div>

    <details v-if="provider.config.length" class="card__config">
      <summary>
        Configuration
        <span>{{ provider.config.length }} field{{ provider.config.length === 1 ? '' : 's' }}</span>
      </summary>
      <ul class="config-fields">
        <li
          v-for="field in provider.config"
          :key="field.name"
          class="config-field"
          :data-config-of="provider.id"
          :data-config-field="field.name"
        >
          <div class="config-field__head">
            <strong>{{ field.label }}</strong>
            <span class="config-field__chip">{{ field.level }}</span>
            <span v-if="field.secret" class="config-field__chip">secret</span>
            <span v-if="field.approval === 'operator'" class="config-field__chip">
              operator approval required
            </span>
            <span v-if="field.required === false" class="config-field__chip">optional</span>
          </div>
          <p>{{ field.help }}</p>
          <p class="config-field__meta">
            <span>Input: {{ field.format ?? 'text' }}</span>
            <span v-if="field.example">Example: <code>{{ field.example }}</code></span>
          </p>
          <ul v-if="field.choices?.length" class="config-field__choices">
            <li v-for="choice in field.choices" :key="choice.value">
              {{ choice.label }} · <code>{{ choice.value }}</code>
            </li>
          </ul>
          <a v-if="field.docs_url" :href="field.docs_url">Vendor setup documentation</a>
        </li>
      </ul>
      <p v-if="provider.verify" class="config-verify" :data-verify-of="provider.id">
        Test connection with <code>{{ provider.verify }}</code>
      </p>
    </details>
    <p
      v-if="provider.verify && !provider.config.length"
      class="config-verify"
      :data-verify-of="provider.id"
    >
      Test connection with <code>{{ provider.verify }}</code>
    </p>

    <!-- Only for a connector that describes one: a heading over an empty list would tell a visitor
         that sixteen connectors have an inbound surface they have not filled in, when in fact they
         declare none. -->
    <InboundSurface v-if="hasInboundSurface(provider)" :provider="provider" />

    <IssueNotice
      title="Connector-wide availability limitation"
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

/*
 * The header is what set the floor on card width, and therefore on how many columns the grid could
 * have. Vendor name, id and status badge sat on one unwrappable flex line whose min-content came to
 * 274px, so a card could not go below 314px and the grid could not fit a fourth track in 1025px.
 * Letting it wrap removes that floor: the badge drops to its own line on a narrow card instead of
 * escaping the border, and the header is unchanged on a wide one, where it still fits on one line.
 */
.card__head {
  display: flex;
  flex-wrap: wrap;
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
  /* A single value with no break opportunity of its own must break rather than push its grid track
     wider than the card. This is the *within a value* half; `.card__hosts` below is the *between
     values* half. Its reach is exactly this `<dl>` — so it covers the Address fact, and it does NOT
     cover a service's gid, which lives in `.card__services` below. That one needs no cover today:
     `.service` already wraps between its chips, and a review measured no spill from it at 1280,
     1366 or 1440. If a long gid ever does escape, this is the rule to mirror there. */
  overflow-wrap: anywhere;
}

/* The hosts cell renders one `<code>` per host with **no whitespace between them**, so the markup
   offers no soft-wrap opportunity: two adjacent hostnames concatenate into one unbreakable ~298px
   inline box. The 609px single-column card the 688px page forced absorbed it; the 424.5px
   two-column track C-100 introduced does not, and it escaped the page — 29px of horizontal overflow
   at 1280 and 8px at 1366, against 0 at the merge base.
   Wrapping restores the break the missing whitespace never provided, and the gap replaces the
   separation. Scoped to this one cell rather than to `.card__facts dd` so the text-only facts keep
   their block layout. */
.card__hosts {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.card__services {
  margin: 12px 0 0;
}

.card__services-title {
  margin: 0;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--vp-c-text-3);
}

.services {
  display: grid;
  gap: 4px;
  margin: 4px 0 0;
  padding: 0;
  list-style: none;
}

.service {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px 10px;
  margin: 0;
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.service__name {
  font-size: 12px;
  color: var(--vp-c-text-1);
}

.service__version {
  border-radius: 10px;
  padding: 0 8px;
  font-size: 11px;
  line-height: 18px;
  background-color: var(--vp-c-default-soft);
}

.service__gid {
  font-size: 12px;
}

.card__config {
  margin: 12px 0 0;
  border-top: 1px solid var(--vp-c-divider);
  padding-top: 10px;
  font-size: 13px;
}

.card__config summary {
  cursor: pointer;
  font-weight: 600;
}

.card__config summary span {
  margin-left: 6px;
  color: var(--vp-c-text-3);
  font-weight: 400;
}

.config-fields {
  display: grid;
  gap: 10px;
  margin: 10px 0 0;
  padding: 0;
  list-style: none;
}

.config-field {
  margin: 0;
  border-left: 2px solid var(--vp-c-divider);
  padding-left: 10px;
}

.config-field__head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px 6px;
}

.config-field__chip {
  border-radius: 10px;
  padding: 0 7px;
  background-color: var(--vp-c-default-soft);
  color: var(--vp-c-text-2);
  font-size: 10px;
  line-height: 17px;
}

.config-field p {
  margin: 3px 0 0;
  color: var(--vp-c-text-2);
  line-height: 1.45;
}

.config-field__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  font-size: 11px;
}

.config-field__choices {
  margin: 4px 0 0;
  padding-left: 16px;
  color: var(--vp-c-text-2);
}

.config-field a {
  display: inline-block;
  margin-top: 4px;
}

.config-verify {
  margin: 10px 0 0;
  color: var(--vp-c-text-2);
}

.card__warn {
  color: var(--vp-c-danger-1);
  font-weight: 600;
}

/* C-408. A field this source did not publish is not a defect and must not be dressed as one: the
   muted text colour, no weight, no danger. The tone is the difference between "this connector has
   no auth" and "this document does not carry auth", which is the whole story. */
.card__unpublished {
  color: var(--vp-c-text-3);
  font-style: italic;
}

.card__ok {
  color: var(--vp-c-text-1);
}
</style>
