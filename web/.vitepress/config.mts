import { defineConfig } from 'vitepress'

const repo = 'https://github.com/codewandler/flux-connectors'
const base = '/flux-connectors/'

export default defineConfig({
  lang: 'en-US',
  title: 'flux-connectors',
  description: 'A catalogue of typed SaaS operations for Flux.',

  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: `${base}brand/icon.svg` }]],

  // A *project* Pages site is served from https://codewandler.github.io/flux-connectors/, and every
  // asset URL and root-relative link is resolved against this prefix. It must match the repository
  // name exactly: with the default '/' the deployed site loads its own JS from the wrong origin path
  // and renders a blank page. Change this only alongside a rename or a custom domain.
  base,

  cleanUrls: true,

  // web/README.md documents how to build this site for a contributor; it is not a published page.
  // Without this it renders at /flux-connectors/README.
  srcExclude: ['README.md'],

  // Dead internal links fail the build rather than shipping. Combined with the Pages workflow, that
  // means a broken site cannot publish silently.
  ignoreDeadLinks: false,

  // Note: ```flux blocks log "The language 'flux' is not loaded, falling back to 'txt'" on every
  // build. That is expected — shiki has no Flux grammar. The fence is left as `flux` rather than
  // `txt` so the blocks light up for free once a grammar exists; do NOT "fix" the warning with
  // `markdown.languageAlias`, which turns the warning into a hard "Language `flux` not found"
  // build failure, and do not alias it to a lookalike language, which colours Flux by another
  // language's rules.

  themeConfig: {
    logo: { src: '/brand/icon.svg', alt: 'flux-connectors' },
    nav: [
      { text: 'Connectors', link: '/explorer' },
      { text: 'v0.1.0', link: `${repo}/releases` },
    ],

    sidebar: [
      {
        text: 'flux-connectors',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Connector & operation explorer', link: '/explorer' },
        ],
      },
    ],

    socialLinks: [{ icon: 'github', link: repo }],

    footer: {
      message: 'Dual-licensed under MIT or Apache-2.0, at your option.',
      copyright: `<a href="${repo}">codewandler/flux-connectors</a>`,
    },

    outline: [2, 3],
  },
})
