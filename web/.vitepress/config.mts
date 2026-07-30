import { defineConfig } from 'vitepress'

const repo = 'https://github.com/codewandler/flux-connectors'
const base = '/'

export default defineConfig({
  lang: 'en-US',
  title: 'flux-connectors',
  description: 'A catalogue of typed SaaS operations for Flux.',

  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: `${base}brand/icon.svg` }]],

  // The committed CNAME publishes this site at flux.codewandler.org, so assets and specification
  // URLs resolve from the origin root rather than from a GitHub project-pages prefix.
  base,

  cleanUrls: true,

  // web/README.md documents how to build this site for a contributor; it is not a published page.
  // Without this it renders at /README.
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
    logo: { src: '/brand/icon.svg', alt: '' },
    nav: [
      { text: 'Connectors', link: '/explorer' },
      { text: 'v0.1.0', link: `${repo}/releases` },
    ],

    sidebar: [
      {
        text: 'flux-connectors',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Connector & core explorer', link: '/explorer' },
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
