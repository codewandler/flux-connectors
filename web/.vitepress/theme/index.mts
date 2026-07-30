// The default theme plus the explorer's components, registered globally so a markdown page can use
// them without an import. Nothing else is customised — the site is documentation with an interactive
// index over generated data, not a bespoke app.

import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'

import CatalogExplorer from './components/CatalogExplorer.vue'
import CatalogSnapshot from './components/CatalogSnapshot.vue'
import OperationDetail from './components/OperationDetail.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('CatalogExplorer', CatalogExplorer)
    app.component('CatalogSnapshot', CatalogSnapshot)
    app.component('OperationDetail', OperationDetail)
  },
} satisfies Theme
