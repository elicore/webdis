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
            { label: 'Intent', link: '/getting-started/intent/' },
            { label: 'Run', link: '/getting-started/run/' },
            { label: 'First Requests', link: '/getting-started/first-requests/' }
          ]
        },
        {
          label: 'Guides',
          items: [
            { label: 'Docker Dev', link: '/guides/docker-dev/' },
            { label: 'Docker Production', link: '/guides/docker-prod/' },
            { label: 'Embedding', link: '/guides/embedding/' },
            { label: 'Security', link: '/guides/security/' }
          ]
        },
        {
          label: 'Reference',
          items: [
            { label: 'CLI', link: '/reference/cli/' },
            { label: 'HTTP', link: '/reference/http/' },
            { label: 'WebSocket', link: '/reference/websocket/' },
            { label: 'Response Formats', link: '/reference/formats/' },
            { label: 'Configuration', link: '/reference/configuration/' },
            { label: 'Config Examples', link: '/reference/config-examples/' }
          ]
        },
        {
          label: 'Compatibility',
          items: [
            { label: 'Webdis Compatibility Scope', link: '/compatibility/webdis-compatibility/' },
            { label: 'Migration Guide', link: '/compatibility/migration-webdis-to-redis-web/' },
            { label: 'Compatibility Test Matrix', link: '/compatibility/compat-test-matrix/' }
          ]
        },
        {
          label: 'Maintainers',
          items: [
            { label: 'Architecture', link: '/maintainers/architecture/' },
            { label: 'Testing and CI', link: '/maintainers/testing/' },
            { label: 'Release and Signing', link: '/maintainers/release/' },
            { label: 'Changelog', link: '/maintainers/changelog/' }
          ]
        }
      ]
    })
  ]
});
