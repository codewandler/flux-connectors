import { defineConfig } from 'vitepress'

const repo = 'https://github.com/codewandler/flux-connectors'
const base = '/flux-connectors/'

export default defineConfig({
  lang: 'en-US',
  title: 'flux-connectors',
  description: 'A catalogue of typed SaaS operations for Flux.',

  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: `${base}brand/icon.svg` }]],

  // This must match where GitHub actually serves the site, which is
  // https://codewandler.github.io/flux-connectors/ — every asset URL and root-relative link resolves
  // against it, so a wrong prefix 404s the stylesheet and the page renders unstyled.
  //
  // `web/public/CNAME` names flux.codewandler.org and this was briefly set to '/' to match. That was
  // premature: the Pages API still reports `"cname": null` and serves the project-pages URL, and
  // flux.codewandler.org resolves to 35.159.24.21, which is not one of GitHub's Pages addresses
  // (185.199.108-111.153). So the custom domain is not live, and '/' 404s every asset.
  //
  // Flip this to '/' **only** once `gh api repos/codewandler/flux-connectors/pages` reports the
  // cname — not when the CNAME file lands, which is what went wrong.
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
      { text: 'v0.4.0', link: `${repo}/releases` },
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
