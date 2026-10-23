import { expect, test } from 'claude-code/testing'

import type { Step } from '../types'
import { ROWS, describe, goalX, heroX, newWorld, paint, simulate, toBase64 } from './scene'
import type { Frame, Scene, World } from './scene'

const step = (kind: Step['kind'], status: Step['status'] = 'ok'): Step => ({
  id: kind + Math.random(), kind, label: kind, file: null, status, at: 0,
})

const text = (cells: Uint32Array, columns: number) => {
  const rows: string[] = []
  for (let r = 0; r < ROWS; r++) {
    let line = ''
    for (let c = 0; c < columns; c++) line += String.fromCodePoint(cells[(r * columns + c) * 3] ?? 32)
    rows.push(line)
  }
  return rows.join('\n')
}
const colors = (cells: Uint32Array) => new Set(cells.filter((_, i) => i % 3 !== 0))
const log = (s: string) => (globalThis as unknown as { console: { log(s: string): void } }).console.log('\n' + s + '\n')

/** Прогнать мир n кадров по 0,05 с, вернуть максимальную высоту прыжка */
const run = (w: World, sc: Scene, n: number, each?: (w: World) => void) => {
  let top = 0
  for (let i = 0; i < n; i++) {
    simulate(w, 0.05, sc)
    top = Math.max(top, w.hero.y)
    each?.(w)
  }
  return top
}
const draw = (f: Partial<Frame> & Scene & { world: World }) => paint({ t: 1000, isThinking: false, ...f })

test('подписи шагов по-русски', () => {
  expect(describe({ tool: 'Edit', file_path: '/a/b/register.ts' }).label).toBe('Правлю register.ts')
  expect(describe({ tool: 'Bash', command: 'ls', description: 'Список файлов' })).toEqual({
    kind: 'bash', label: 'Список файлов', file: null,
  })
  expect(describe({ tool: 'mcp__telegram__send_message' }).label).toBe('telegram: send_message')
})

test('физика: добегает до станции, прыгает, сплющивается при приземлении', () => {
  const columns = 60
  expect(heroX(columns, 3) > heroX(columns, 2)).toBe(true)
  expect(heroX(columns, 500) < goalX(columns) - 5).toBe(true) // без конца хода до цели не доходит
  const w = newWorld()
  const sc: Scene = { columns, steps: [step('read'), step('edit')], goal: false, busy: true }
  let squashed = false
  expect(run(w, sc, 80, x => { squashed ||= x.hero.squash > 0 }) > 0.5).toBe(true) // подпрыгнул на станции
  expect(squashed).toBe(true)
  expect(w.hero.y).toBe(0)
  expect(Math.abs(w.hero.x - heroX(columns, 2)) < 0.5).toBe(true)
  expect(w.parts.some(p => p.ch === '∘' || p.y > 3)).toBe(true) // дым из трубы
  for (const p of w.parts) expect(p.y >= 0).toBe(true) // частицы не проваливаются под землю
})

test('ошибка отбрасывает назад и кружит голову, у цели присед и прыжки радости', () => {
  const columns = 60
  const w = newWorld()
  const steps = [step('read'), step('bash')]
  run(w, { columns, steps, goal: false, busy: false }, 80)
  const before = w.hero.x
  steps[1] = { ...steps[1]!, status: 'err' }
  simulate(w, 0.05, { columns, steps, goal: false, busy: false })
  expect(w.hero.x < before).toBe(true)
  expect(w.hero.hurt > 0).toBe(true)
  run(w, { columns, steps, goal: false, busy: false }, 6)
  const pic = text(draw({ columns, steps, goal: false, busy: false, world: w }), columns)
  log(pic)
  expect(pic.includes('✦')).toBe(true) // звёздочки вокруг головы

  let crouchedOnGround = false
  const top = run(w, { columns, steps, goal: true, busy: false }, 120, x => {
    crouchedOnGround ||= x.hero.crouch > 0 && x.hero.y === 0
  })
  expect(crouchedOnGround).toBe(true)
  expect(top > 1).toBe(true)
  expect(w.hero.x > goalX(columns) - 16).toBe(true)
})

test('мультик: домик, Clawd по иконке, клавдики, флажок, ночной колпак', () => {
  const columns = 60
  const steps = [step('read'), step('edit'), step('bash', 'err'), step('web')]
  const w = newWorld([{ id: 'a', x: 12, status: 'run' }, { id: 'b', x: 4, status: 'ok' }])
  run(w, { columns, steps, goal: false, busy: true }, 160)
  const sc = { columns, steps, goal: false, busy: true, world: w }
  const day = draw({ ...sc, hour: 13 })
  const night = draw({ ...sc, hour: 23 })
  for (const cells of [day, night]) {
    expect(cells.length).toBe(columns * ROWS * 3)
    const pic = text(cells, columns)
    log(pic)
    expect(pic.includes('▟████▙')).toBe(true) // крыша
    expect(pic.includes('▄██▄█████▄██▄')).toBe(true) // Clawd: глаза-прорези, руки по бокам
    expect(pic.includes('▝▛▜▘') || pic.includes('▗▛▜▖')).toBe(true) // клавдик
    expect(pic.includes('✕') && pic.includes('┃')).toBe(true) // ошибка на дороге, флажок
  }
  expect(colors(night).has(0x6f9cf5)).toBe(true) // колпак
  expect(colors(day).has(0x6f9cf5)).toBe(false)
  // моргает: глаза закрываются веками
  expect(colors(draw({ ...sc, t: 4100 * 3 + 50 })).has(0xa8543c)).toBe(true)

  const many = Array.from({ length: 80 }, () => step('read'))
  const big = newWorld()
  run(big, { columns: 30, steps: many, goal: false, busy: false }, 100)
  expect(draw({ columns: 30, steps: many, goal: false, busy: false, world: big }).length).toBe(30 * ROWS * 3)
  expect(typeof toBase64(draw({ columns, steps, goal: true, busy: false, world: w }))).toBe('string')
})

test('реквизит под шаг: книга, ноутбук, лупа, глобус', () => {
  const columns = 70
  const at = (kind: Step['kind']) => {
    const steps = [step('read'), step(kind, 'run')]
    const w = newWorld()
    run(w, { columns, steps, goal: false, busy: true }, 160)
    const cells = draw({ columns, steps, goal: false, busy: true, world: w, t: 777 })
    log(text(cells, columns))
    return { cells, pic: text(cells, columns) }
  }
  expect(colors(at('read').cells).has(0xf1ead8)).toBe(true) // страницы книги
  expect(at('bash').pic.includes('$')).toBe(true) // экран ноутбука печатает
  expect(colors(at('search').cells).has(0x8b929c)).toBe(true) // лупа
  expect(/[◐◓◑◒]/.test(at('web').pic)).toBe(true) // глобус
})

test('долго дома: ломится в дверь, часы тикают в кадре', () => {
  const columns = 40
  const sc = { columns, steps: [], goal: false, busy: true, world: newWorld(), isThinking: true }
  const knock = text(draw({ ...sc, t: 3600 * 5 + 50, elapsed: 65_000 }), columns)
  const peek = text(draw({ ...sc, t: 3600 * 5 + 1800, elapsed: 66_000 }), columns)
  log(knock)
  log(peek)
  expect(knock.includes('1:05') && (knock.includes('тук') || knock.includes('бум'))).toBe(true)
  expect(peek.includes('1:06')).toBe(true)
  expect(colors(draw({ ...sc, t: 3600 * 5 + 1800 })).has(0xd97757)).toBe(true) // выглянул из двери
})
