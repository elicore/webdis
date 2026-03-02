import fs from 'node:fs';
import path from 'node:path';

const docsRoot = path.resolve('src/content/docs');
const mdExtensions = new Set(['.md', '.mdx']);

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const next = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(next, out);
      continue;
    }
    if (mdExtensions.has(path.extname(entry.name))) {
      out.push(next);
    }
  }
  return out;
}

function existsForDocTarget(fromFile, target) {
  const noAnchor = target.split('#')[0];
  const trimmed = noAnchor.endsWith('/') ? noAnchor.slice(0, -1) : noAnchor;
  const abs = path.resolve(path.dirname(fromFile), trimmed);

  const candidates = [
    abs,
    `${abs}.md`,
    `${abs}.mdx`,
    path.join(abs, 'index.md'),
    path.join(abs, 'index.mdx')
  ];

  return candidates.some((candidate) => fs.existsSync(candidate));
}

const files = walk(docsRoot);
const errors = [];
const linkRegex = /\[[^\]]+\]\(([^)\s]+)\)/g;

for (const file of files) {
  const content = fs.readFileSync(file, 'utf8');
  for (const match of content.matchAll(linkRegex)) {
    const href = match[1];
    if (
      href.startsWith('http://') ||
      href.startsWith('https://') ||
      href.startsWith('mailto:') ||
      href.startsWith('#')
    ) {
      continue;
    }

    const relative = href.startsWith('/')
      ? path.join(docsRoot, href.slice(1))
      : href;

    if (!existsForDocTarget(file, relative)) {
      errors.push(`${path.relative(process.cwd(), file)} -> ${href}`);
    }
  }
}

if (errors.length > 0) {
  console.error('Broken internal docs links:');
  for (const err of errors) {
    console.error(`- ${err}`);
  }
  process.exit(1);
}

console.log('Internal docs links look valid.');
