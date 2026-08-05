<script setup lang="ts">
// One operation, in full, at its own URL.
//
// Everything here is pre-rendered from the generated catalogue at build time, so the page is
// complete with JavaScript switched off — the signature, the typed parameters, the generated Flux
// verbatim, the credentials and the hosts.
//
// The status is presented at two levels, which is the whole point of the `scope` field:
//
//   - a defect this operation **owns** is a warning block directly under its title, and the badge in
//     every list that mentions it;
//   - a condition it **inherits** from its provider or from the catalogue is a neutral block further
//     down, worded as context. Every operation in the catalogue carries at least one of those today,
//     and none of them is a fault of the operation.
//
// C-206 adds a third level that is not a limitation at all: a published fact about the operation,
// shown under Credentials, where a reader who wants to know what to supply is already looking.

import { computed, inject } from 'vue'
import {
  PATH_RESOLVER,
  UNPUBLISHED,
  identityPath,
  inheritedIssues,
  notes,
  operationCredentials,
  ownIssues,
  ownsDefect,
  providerAuth,
  published,
  signature,
  type Catalog,
  type Credential,
  type Operation,
  type PathResolver,
  type Provider,
} from '../../../data/catalog.mts'
import FluxSource from './FluxSource.vue'
import IssueNotice from './IssueNotice.vue'
import ParameterTable from './ParameterTable.vue'
import SpecChip from './SpecChip.vue'
import StatusBadge from './StatusBadge.vue'

const props = defineProps<{ catalog: Catalog; id: string }>()

const resolvePath = inject<PathResolver>(PATH_RESOLVER, identityPath)

const provider = computed<Provider | undefined>(() =>
  props.catalog.providers.find((candidate) =>
    candidate.operations.some((operation) => operation.id === props.id)
  )
)

const operation = computed<Operation | undefined>(() =>
  provider.value?.operations.find((candidate) => candidate.id === props.id)
)

/** Resolve a credential reference against the provider's declared credentials. */
function credential(name: string): Credential | undefined {
  const auth = provider.value ? providerAuth(provider.value) : null
  return auth?.credentials.find((candidate) => candidate.name === name)
}

/**
 * The credential alternatives this source published, or `null` when it published none (C-408).
 *
 * `[]` and `null` are opposite facts here. An empty list is the C-206 case — a credential withheld,
 * or a vendor that needs none — and the sentence about live calls being disabled is written for it.
 * `null` is a source that carries no credentials at all, and that sentence is simply false of it.
 */
const credentials = computed(() =>
  operation.value ? operationCredentials(operation.value) : null
)

const own = computed(() => (operation.value ? ownIssues(operation.value) : []))
const inherited = computed(() => (operation.value ? inheritedIssues(operation.value) : []))

/**
 * The published facts that are not defects — see `notes` in the catalogue contract.
 *
 * Rendered inside **Credentials**, because that is the heading a reader is under when the question
 * arises: an operation listing no credential is either waiting on one nobody can hold safely yet, or
 * needs none at all, and the page used to answer both with the same sentence about live calls being
 * disabled. The catalogue now says which, so the page stops guessing on its behalf.
 */
const clear = computed(() => (operation.value ? notes(operation.value) : []))

/**
 * The operation's Flux signature, or `null` when this source published no Flux (C-408).
 *
 * Not a cosmetic absence like the others: reading a first line off a module that is not there threw,
 * so a page over a source that carries no Flux did not mislead, it failed to render at all.
 */
const declaration = computed(() => (operation.value ? signature(operation.value) : null))
</script>

<template>
  <p v-if="!operation" class="missing">
    No operation with the id <code>{{ id }}</code> is in the connector catalogue.
  </p>

  <article
    v-else
    class="op"
    :data-operation="operation.id"
    :data-defect="ownsDefect(operation) ? 'own' : 'none'"
    :data-inherited-issues="inherited.length"
    :data-notes="clear.length"
  >
    <p class="op__lede">{{ operation.description }}</p>

    <p class="op__chips">
      <StatusBadge :operation="operation" />
      <!-- C-408. Omitted rather than rendered empty: a chip with nothing in it is a request shape
           this source did not publish, wearing the costume of one it did. -->
      <span v-if="published(operation.method)" class="chip chip--method">{{ operation.method }}</span>
      <code v-if="published(operation.path)" class="chip chip--path">{{ operation.path }}</code>
      <span class="chip">{{ operation.direction }}</span>
      <span class="chip">risk: {{ operation.risk }}</span>
      <span class="chip">{{ operation.idempotency }}</span>
      <a class="chip chip--link" :href="resolvePath(`/explorer#${provider!.id}`)">
        {{ provider!.vendor }}
      </a>
    </p>

    <div class="op__semantic-effects">
      <strong>Semantic effects</strong>
      <span class="op__semantic-effect-chips">
        <SpecChip v-if="!operation.semantic_effects.length" value="none" />
        <SpecChip
          v-for="effect in operation.semantic_effects"
          :key="effect"
          :value="effect"
        />
      </span>
    </div>

    <IssueNotice
      title="Known limitation for this operation"
      tone="defect"
      :issues="own"
    />

    <h2>Signature</h2>
    <FluxSource v-if="declaration" :source="declaration" />
    <p v-else class="op__note op__unpublished">{{ UNPUBLISHED }}</p>

    <h2>Parameters</h2>
    <ParameterTable :parameters="operation.parameters" />

    <template v-if="operation.body_schema">
      <h3>Request body</h3>
      <p class="op__note">
        The body is a schema rather than a set of named fields; it is passed as one value.
      </p>
      <pre class="op__schema"><code>{{ JSON.stringify(operation.body_schema, null, 2) }}</code></pre>
    </template>

    <template v-if="operation.response_schema">
      <h3>Response</h3>
      <pre class="op__schema"><code>{{ JSON.stringify(operation.response_schema, null, 2) }}</code></pre>
    </template>

    <h2>Flux source</h2>
    <template v-if="published(operation.flux)">
      <p class="op__note">
        The exact Flux declaration for this operation. Live installation is not available yet.
      </p>
      <FluxSource :source="operation.flux" />
    </template>
    <p v-else class="op__note op__unpublished">{{ UNPUBLISHED }}</p>

    <h2>Credentials</h2>
    <IssueNotice title="Nothing to supply here" tone="clear" :issues="clear" />
    <!-- C-408. The condition's first term is the whole fix. The sentence below is true of an
         operation whose catalogue published a credential set and left it empty — a credential
         withheld because nobody can hold it safely yet — and says nothing true about a source that
         publishes no credentials at all. It stays exactly as it was for the first. -->
    <p v-if="credentials && !credentials.length && !clear.length" class="op__note">
      No safe credential configuration is available for this operation. Live calls are disabled.
    </p>
    <p v-else-if="!credentials" class="op__note op__unpublished">{{ UNPUBLISHED }}</p>
    <template v-else-if="credentials.length">
      <p class="op__note">
        Alternatives, any one of which authenticates the request. Each alternative lists every
        credential that must be supplied together. Environment variables are named, never read — no
        credential value appears in any generated artifact.
      </p>
      <ul class="creds">
        <li v-for="(alternative, index) in credentials" :key="index" class="creds__alt">
          <ul class="creds__and">
            <li v-for="name in alternative" :key="name">
              <code>{{ name }}</code>
              <template v-if="credential(name)">
                <span class="creds__scheme">
                  {{ credential(name)!.scheme.kind
                  }}<template v-if="credential(name)!.scheme.name">
                    ({{ credential(name)!.scheme.name }})</template
                  >
                </span>
                <div class="creds__desc">{{ credential(name)!.description }}</div>
                <div class="creds__env">
                  from
                  <code v-for="variable in credential(name)!.env" :key="variable">{{ variable }}</code>
                  <template v-if="credential(name)!.user_env.length">
                    · username from
                    <code v-for="variable in credential(name)!.user_env" :key="variable">{{
                      variable
                    }}</code>
                  </template>
                  <template v-if="credential(name)!.user_suffix">
                    · username suffix <code>{{ credential(name)!.user_suffix }}</code>
                  </template>
                  <template v-if="credential(name)!.oauth2"> · acquired by OAuth2 </template>
                </div>
              </template>
            </li>
          </ul>
        </li>
      </ul>
    </template>

    <h2>Hosts</h2>
    <ul class="hosts">
      <li v-for="host in operation.hosts" :key="host"><code>{{ host }}</code></li>
    </ul>
    <p v-if="published(provider!.base_url)" class="op__note">
      Base URL <code>{{ provider!.base_url }}</code>
    </p>

    <IssueNotice
      title="Current availability constraints"
      tone="inherited"
      :issues="inherited"
    />
  </article>
</template>

<style scoped>
.op__semantic-effects {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin: 14px 0;
}

.op__semantic-effect-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.op__lede {
  font-size: 16px;
  color: var(--vp-c-text-2);
  margin: 0 0 12px;
}

.op__chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin: 0 0 8px;
}

.chip {
  display: inline-block;
  border-radius: 10px;
  padding: 1px 10px;
  font-size: 12px;
  line-height: 20px;
  background-color: var(--vp-c-default-soft);
  color: var(--vp-c-text-2);
  white-space: nowrap;
}

.chip--method {
  font-weight: 600;
  color: var(--vp-c-text-1);
}

.chip--path {
  font-family: var(--vp-font-family-mono);
}

.chip--link {
  color: var(--vp-c-brand-1);
  text-decoration: none;
}

.op__note {
  font-size: 14px;
  color: var(--vp-c-text-2);
}

/* C-408. A statement about the document, not about the operation — so it is muted and never toned
   as a defect. What it replaces was a sentence claiming live calls were disabled. */
.op__unpublished {
  color: var(--vp-c-text-3);
  font-style: italic;
}

.op__schema {
  background-color: var(--vp-c-bg-soft);
  border-radius: 8px;
  padding: 12px 16px;
  overflow-x: auto;
}

.op__schema code {
  font-size: 12px;
  white-space: pre;
}

.creds,
.hosts {
  list-style: none;
  padding: 0;
  margin: 0;
}

.creds__alt + .creds__alt::before {
  content: 'or';
  display: block;
  font-size: 12px;
  color: var(--vp-c-text-3);
  margin: 8px 0;
}

.creds__and {
  list-style: none;
  padding: 0;
  margin: 0;
}

.creds__and > li {
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 10px 14px;
  margin: 8px 0;
}

.creds__scheme {
  margin-left: 8px;
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.creds__desc {
  font-size: 14px;
  margin-top: 4px;
}

.creds__env {
  font-size: 12px;
  color: var(--vp-c-text-2);
  margin-top: 4px;
}

.missing {
  color: var(--vp-c-danger-1);
}
</style>
