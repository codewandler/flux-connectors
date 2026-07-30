---
outline: [2, 2]
prev: false
next: false
---

<script setup>
import { useData } from 'vitepress'
import { data as catalog } from '../data/catalog.data.mts'

const { params } = useData()
</script>

<!-- @content -->

<OperationDetail :catalog="catalog" :id="params.operation" />

<p class="op-back"><a href="../explorer">← All connectors and operations</a></p>

<style scoped>
.op-back {
  margin-top: 32px;
  font-size: 14px;
}
</style>
