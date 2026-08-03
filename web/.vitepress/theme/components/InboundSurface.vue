<script setup lang="ts">
// A connector's **inbound** surface: the events a vendor sends, and the bindings that make one
// reachable (C-83).
//
// The site described half of what a connector does until this existed. An operation is flux calling
// the vendor and had a page, a row and a filter; an event is the vendor calling flux and had nothing
// at all, because until C-83 it reached no artifact for a site to read.
//
// **A binding is not rendered as an operation, because it is not one.** It declares and never
// installs: it names no URL, no schedule and no secret, and it is emitted into no Flux module. So
// there is no signature to show and no "call this" affordance. What a visitor needs is what it
// listens for, how a delivery is proven, and what answers — which is exactly what is below.
//
// The verification chip is the one part worth being careful about. It reads `verification.verified`,
// a value the catalogue publishes on **every** binding, rather than checking whether an `hmac` block
// happens to be present. Deciding by absence is how "nothing arrives unsolicited over this socket"
// and "anyone can POST to this endpoint" come to render identically.

import { computed } from 'vue'
import {
  channelEvents,
  replyAddress,
  verificationLabel,
  type Provider,
} from '../../../data/catalog.mts'

const props = defineProps<{ provider: Provider }>()

/** Events nothing in this connector's own bindings carries — declared, but not delivered here. */
const unbound = computed(() => {
  const carried = new Set(props.provider.channels.flatMap((channel) => channel.events))
  return props.provider.events.filter((event) => !carried.has(event.name))
})
</script>

<template>
  <div class="inbound" :data-inbound-of="provider.id">
    <h4 class="inbound__title">Inbound</h4>

    <ul class="bindings">
      <li
        v-for="channel in provider.channels"
        :key="channel.name"
        class="binding"
        :data-channel="channel.name"
        :data-channel-of="provider.id"
        :data-transport="channel.transport"
        :data-verified="String(channel.verification.verified)"
        :data-payload-root="String(channel.payload_root)"
      >
        <div class="binding__head">
          <code class="binding__name">{{ channel.name }}</code>
          <span class="binding__transport">{{ channel.transport }}</span>
          <span
            class="binding__chip"
            :class="channel.verification.verified ? 'binding__chip--ok' : 'binding__chip--warn'"
          >
            {{ verificationLabel(channel) }}
          </span>
          <span class="binding__payload">
            {{ channel.payload_root ? 'Complete payload' : 'Mapped payload' }}
          </span>
        </div>

        <p v-if="channel.description" class="binding__desc">{{ channel.description }}</p>

        <ul v-if="channelEvents(provider, channel).length" class="events">
          <li
            v-for="event in channelEvents(provider, channel)"
            :key="event.name"
            class="event"
            :data-event="event.name"
          >
            <code class="event__name">{{ event.name }}</code>
            <span v-if="event.wire_value" class="event__wire">
              wire <code>{{ event.wire_value }}</code>
            </span>
            <span v-if="!event.default" class="event__off">off by default</span>
          </li>
        </ul>

        <details v-if="channel.connect" class="binding__connect">
          <summary>Connection contract</summary>
          <dl>
            <div>
              <dt>Path</dt>
              <dd><code>{{ channel.connect.path }}</code></dd>
            </div>
            <div v-if="Object.keys(channel.connect.query ?? {}).length">
              <dt>Query</dt>
              <dd>
                <code v-for="(value, name) in channel.connect.query" :key="name">
                  {{ name }}={{ value }}
                </code>
              </dd>
            </div>
            <div v-if="channel.connect.auth?.length">
              <dt>Credentials</dt>
              <dd>
                <code v-for="(alternative, index) in channel.connect.auth" :key="index">
                  {{ alternative.credentials.join(' + ') }}
                </code>
              </dd>
            </div>
            <div v-if="channel.connect.subprotocols?.length">
              <dt>Subprotocols</dt>
              <dd><code>{{ channel.connect.subprotocols.join(', ') }}</code></dd>
            </div>
          </dl>
        </details>

        <p v-if="replyAddress(channel)" class="binding__reply">
          Replies with <code>{{ replyAddress(channel) }}</code>
        </p>
      </li>
    </ul>

    <div v-if="unbound.length" class="inbound__unbound">
      <span class="inbound__unbound-label">Also declared</span>
      <ul class="events">
        <li v-for="event in unbound" :key="event.name" class="event" :data-event="event.name">
          <code class="event__name">{{ event.name }}</code>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.inbound {
  margin: 12px 0 0;
}

.inbound__title,
.inbound__unbound-label {
  margin: 0;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--vp-c-text-3);
}

.bindings,
.events {
  margin: 4px 0 0;
  padding: 0;
  list-style: none;
}

.bindings {
  display: grid;
  gap: 8px;
}

.binding {
  margin: 0;
}

/* Wraps for the reason the card header does: three chips on one unwrappable line would set a floor
   under the card's width, and the explorer's grid drops a column when one does. */
.binding__head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px 8px;
}

.binding__name {
  font-size: 12px;
  color: var(--vp-c-text-1);
}

.binding__transport {
  font-size: 11px;
  line-height: 18px;
  border-radius: 10px;
  padding: 0 8px;
  background-color: var(--vp-c-default-soft);
  color: var(--vp-c-text-2);
}

.binding__chip {
  font-size: 11px;
  font-weight: 600;
  line-height: 18px;
  border-radius: 10px;
  padding: 0 8px;
  white-space: nowrap;
}

.binding__payload {
  font-size: 11px;
  color: var(--vp-c-text-3);
}

.binding__chip--ok {
  background-color: var(--vp-c-tip-soft);
  color: var(--vp-c-tip-1);
}

.binding__chip--warn {
  background-color: var(--vp-c-warning-soft);
  color: var(--vp-c-warning-1);
}

.binding__desc,
.binding__reply {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.binding__connect {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.binding__connect summary {
  cursor: pointer;
  color: var(--vp-c-text-2);
}

.binding__connect dl {
  display: grid;
  gap: 4px;
  margin: 6px 0 0;
}

.binding__connect dl > div {
  display: grid;
  grid-template-columns: 74px minmax(0, 1fr);
  gap: 8px;
}

.binding__connect dt {
  color: var(--vp-c-text-3);
}

.binding__connect dd {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 8px;
  min-width: 0;
  margin: 0;
}

.binding__connect code,
.event__wire code {
  overflow-wrap: anywhere;
}

.binding__reply code,
.event__name {
  font-size: 12px;
  /* A rendered oip has no break opportunity of its own and would otherwise push its card wider. */
  overflow-wrap: anywhere;
}

.events {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 8px;
}

.event {
  display: flex;
  align-items: baseline;
  gap: 4px;
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.event__off {
  font-size: 11px;
  line-height: 18px;
  border-radius: 10px;
  padding: 0 8px;
  background-color: var(--vp-c-warning-soft);
  color: var(--vp-c-warning-1);
}

.event__wire {
  font-size: 11px;
  color: var(--vp-c-text-3);
}

.inbound__unbound {
  margin: 8px 0 0;
}
</style>
