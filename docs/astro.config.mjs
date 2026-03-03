// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  integrations: [
    starlight({
      title: 'redis-web Docs',
      description: 'HTTP and WebSocket gateway for Redis with Webdis compatibility.',
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Overview', link: '/getting-started/overview/' },
            { label: 'Run and First Requests', link: '/getting-started/run-and-first-requests/' }
          ]
        },
        {
          label: 'Guides',
          items: [
            { label: 'Deployment (Docker and Security)', link: '/guides/deployment/' },
            { label: 'Embedding', link: '/guides/embedding/' },
            { label: 'Hiredis Functional Tests', link: '/guides/hiredis-functional-tests/' },
            { label: 'Hiredis Performance Harness', link: '/guides/hiredis-performance-tests/' }
          ]
        },
        {
          label: 'Reference',
          items: [
            { label: 'CLI', link: '/reference/cli/' },
            { label: 'HTTP and WebSocket API', link: '/reference/api/' },
            { label: 'Configuration', link: '/reference/configuration/' }
          ]
        },
        {
          label: 'Compatibility',
          items: [
            { label: 'Webdis Compatibility and Migration', link: '/compatibility/webdis-compatibility/' },
            { label: 'Hiredis Drop-In Compatibility', link: '/compatibility/hiredis-dropin/' }
          ]
        },
        {
          label: 'Maintainers',
          items: [
            { label: 'Maintainers Guide', link: '/maintainers/architecture/' }
          ]
        }
      ]
    })
  ]
});
