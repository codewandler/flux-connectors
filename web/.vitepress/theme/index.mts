// The default theme plus the explorer's components, registered globally so a markdown page can use
// them without an import. Nothing else is customised — the site is documentation with an interactive
// index over generated data, not a bespoke app.
//
// **This file is the VitePress adapter, and since C-142 it is the only one.** No component under
// `components/` imports the framework; the one thing they needed from it — turning a site-root path
// into an href under the deployed base — arrives through `provide`, and defaults to identity for a
// host that supplies nothing. `components/README.md` records the tiers that boundary creates.

import DefaultTheme from 'vitepress/theme'
import { withBase, type Theme } from 'vitepress'
import './custom.css'

import { PATH_RESOLVER } from '../../data/catalog.mts'
import CatalogExplorer from './components/CatalogExplorer.vue'
import CatalogSnapshot from './components/CatalogSnapshot.vue'
import CoreDetail from './components/CoreDetail.vue'
import OperationDetail from './components/OperationDetail.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // `withBase` is a plain function over the site data module, not a composable, so it carries
    // across the provide intact — including through the server render.
    app.provide(PATH_RESOLVER, withBase)

    app.component('CatalogExplorer', CatalogExplorer)
    app.component('CatalogSnapshot', CatalogSnapshot)
    app.component('CoreDetail', CoreDetail)
    app.component('OperationDetail', OperationDetail)
  },
} satisfies Theme
