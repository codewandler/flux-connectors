---
outline: [2, 2]
prev: false
next: false
---

<script setup>
import { useData } from 'vitepress'
import { data as catalog } from '../../data/catalog.data.mts'

const { params } = useData()
</script>

<!-- @content -->

<CoreDetail :catalog="catalog" :kind="params.kind" :name="params.name" />

<p class="core-back"><a href="../../../explorer#core">← All Flux core entries</a></p>

<style scoped>
.core-back {
  margin-top: 32px;
  font-size: 14px;
}
</style>
