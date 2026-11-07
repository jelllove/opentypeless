import fs from 'node:fs'
import path from 'node:path'
import crypto from 'node:crypto'

// Run from the project root; source files are untrusted data, never executed.
const [jiebaDir, acronymsDir, outputDir] = process.argv.slice(2)
if (!jiebaDir || !acronymsDir || !outputDir) throw new Error('Usage: node scripts/build-bundled-dictionary.mjs <jieba staging> <acronym staging> <output>')
const read = (dir, file) => fs.readFileSync(path.join(dir, file), 'utf8')
const chinese = read(jiebaDir, 'dict.txt').split(/\r?\n/).map((line) => {
  const [word, frequency] = line.trim().split(/\s+/)
  return { word, frequency: Number(frequency) }
}).filter(({ word, frequency }) => /^[\p{Script=Han}]{2,8}$/u.test(word ?? '') && Number.isFinite(frequency))
  .sort((a, b) => b.frequency - a.frequency || a.word.localeCompare(b.word)).slice(0, 2400).map(({ word }) => word)
const alphabetSection = read(acronymsDir, 'README.md').split('## Ordered by alphabet')[1]?.split('## Group by')[0] ?? ''
const technical = [...alphabetSection.matchAll(/^- \*\*([^*]+)\*\*/gm)].flatMap((match) => match[1].split(' / ').map((word) => word.trim()))
  .filter((word) => /^[A-Za-z0-9][A-Za-z0-9.+/#-]{1,24}$/.test(word))
const original = ['Azure', 'Azure Speech', 'Azure OpenAI', 'Microsoft Foundry', 'OpenAI', 'GPT', 'Gemini', 'Visual Studio Code', 'TypeScript', 'JavaScript', 'Python', 'Rust', 'GitHub', 'Copilot', 'Kubernetes', 'Docker', 'OpenTypeless', '语音识别', '人工智能', '大语言模型', '提示词', '词典', '快捷键']
const terms = [...new Map([...original, ...technical, ...chinese].map((word) => [word.normalize('NFKC').toLowerCase(), word])).values()]
fs.mkdirSync(outputDir, { recursive: true })
fs.writeFileSync(path.join(outputDir, 'starter-terms.txt'), terms.join('\n') + '\n')
fs.writeFileSync(path.join(outputDir, 'jieba-MIT-LICENSE.txt'), read(jiebaDir, 'LICENSE'))
fs.writeFileSync(path.join(outputDir, 'acronyms-CC0-LICENSE.txt'), read(acronymsDir, 'LICENSE'))
const sha = (dir, file) => crypto.createHash('sha256').update(fs.readFileSync(path.join(dir, file))).digest('hex')
fs.writeFileSync(path.join(outputDir, 'sources.json'), JSON.stringify({
  pack: 'starter-v1', termCount: terms.length, chineseSelectionLimit: 2400,
  sources: [
    { repository: 'https://github.com/fxsjy/jieba', revision: '67fa2e36e72f69d9134b8a1037b83fbb070b9775', file: 'jieba/dict.txt', sha256: sha(jiebaDir, 'dict.txt'), license: 'MIT' },
    { repository: 'https://github.com/wuyichen24/software-engineering-abbreviations-acronyms', revision: '8e1d77e99b184106d50a180ba52cb3557273af6d', file: 'README.md', sha256: sha(acronymsDir, 'README.md'), license: 'CC0-1.0' },
  ], originalTermCount: original.length,
}, null, 2) + '\n')
console.log(JSON.stringify({ terms: terms.length, chinese: chinese.length, technical: technical.length }))
