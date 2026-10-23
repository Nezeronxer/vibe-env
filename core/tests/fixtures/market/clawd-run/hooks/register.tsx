// Clawd бежит: пока Claude работает, вместо спиннера мультик-история - Clawd бежит из домика
// по станциям-шагам к флажку с целью хода, субагенты клавдиками за ним. /clawd - все шаги сессии.
import { atom, read, update } from 'claude-code'
import type { Register } from 'claude-code'

import type { Step } from '../types'
import { COLOR, DOOR_X, ERR, ICON, ROWS, cut, describe, newWorld, paint, simulate, toBase64 } from './scene'

const PANE = 'clawd-run'
const turn = atom({ plugin: 'clawd-run', key: 'turn' } as const, [])
const session = atom({ plugin: 'clawd-run', key: 'session' } as const, [])
const helpers = atom({ plugin: 'clawd-run', key: 'helpers' } as const, 0)

const hex = (c: number) => '#' + c.toString(16).padStart(6, '0')
const colorOf = (s: Step) => hex(s.status === 'err' ? ERR : COLOR[s.kind])
const MODE_LABEL: Record<string, string> = {
  thinking: 'Думаю',
  requesting: 'Жду модель',
  responding: 'Пишу ответ',
  'tool-input': 'Готовлю шаг',
  'tool-use': 'Работаю',
}

export const register: Register = on => {
  // Анимация живёт в переменных модуля: её кадры идут мимо $.state, через blit
  let spinner: string | null = null
  let columns = 60
  let steps: Step[] = []
  let world = newWorld()
  let sent = '' // последний отправленный кадр: одинаковые не шлём
  let inFlight = false
  let isThinking = false
  let goal = false // пишет ответ - бежит к флажку
  let respondingSince = 0
  let goalText = ''
  let turnStart = 0
  const scene = () => ({ columns, steps, goal, busy: isThinking || steps.some(s => s.status === 'run') })
  const frame = () => toBase64(paint({ ...scene(), world, t: Date.now(), isThinking, hour: new Date().getHours(), elapsed: turnStart ? Date.now() - turnStart : undefined }))

  on('session.start', async ($, e, next) => {
    await $.command.register({ name: 'clawd', description: 'Что сделал Clawd: все шаги и файлы этой сессии' })
    // 20 кадров в секунду; кадр уходит, только если изменился и предыдущий уже принят
    $.clock.every(50, () => {
      if (spinner === null) return
      simulate(world, 0.05, scene())
      if (inFlight) return
      const cells = frame()
      if (cells === sent) return
      sent = cells
      inFlight = true
      void $.ui.blit({ requestId: spinner, key: 'scene', cells, columns, rows: ROWS }).then(r => {
        inFlight = false
        if ('deny' in r && r.deny) spinner = null
      }, () => {
        inFlight = false
      })
    })
    return next(e)
  })

  on('command.run', { command: 'clawd' }, async $ => {
    await $.ui.open({ id: PANE, title: 'Что сделал Clawd' })
    return { text: 'Панель «Что сделал Clawd» открыта.' }
  })

  on('prompt.submit', async ($, e, next) => {
    steps = []
    world = newWorld(world.minis) // каждый ход начинается из домика
    turnStart = Date.now()
    goalText = e.text.trim().split('\n')[0] ?? ''
    await update($, turn, () => [])
    return next(e)
  })

  on('agent.spawn', async ($, e, next) => {
    const r = await next(e)
    if (r.agentId) {
      world.minis = [...world.minis, { id: r.agentId, x: DOOR_X, status: 'run' }]
      await update($, helpers, n => n + 1)
    }
    return r
  })

  on('turn.complete', async ($, e, next) => {
    const r = await next(e)
    if (e.agentId) {
      const status = e.reason === 'error' || e.reason === 'aborted' ? 'err' : 'ok'
      world.minis = world.minis.map(m => (m.id === e.agentId ? { ...m, status } : m))
    } else {
      world.minis = world.minis.map(m => ({ ...m, status: m.status === 'run' ? 'ok' : m.status }))
    }
    return r
  })

  on('tool.call', async ($, e, next) => {
    if (e.agentId !== undefined) return next(e) // путь только главного агента
    const d = describe(e as unknown as Record<string, unknown>)
    const step: Step = { id: e.tool_use_id ?? String(Math.random()), ...d, status: 'run', at: Date.now() }
    steps = [...steps, step]
    await update($, turn, list => [...list, step])
    const ran = await next(e)
    const status: Step['status'] = ('deny' in ran && ran.deny) || ran.isError ? 'err' : 'ok'
    const done = { ...step, status }
    steps = steps.map(s => (s.id === step.id ? done : s))
    await update($, turn, list => list.map(s => (s.id === step.id ? done : s)))
    await update($, session, list => [...list, done].slice(-500))
    return ran
  })

  on('ui.render', { component: 'Spinner' }, async ($, e, next) => {
    const list = await read($, turn)
    const live = list.at(-1)
    isThinking = e.props.mode === 'thinking' || e.props.mode === 'requesting'
    // к флажку только на настоящем ответе: короткая реплика перед шагом его не дёргает
    if (e.props.mode !== 'responding') respondingSince = 0
    else if (respondingSince === 0) respondingSince = Date.now()
    goal = respondingSince > 0 && Date.now() - respondingSince > 1200
    const label = live?.status === 'run' ? live.label : (MODE_LABEL[e.props.mode] ?? 'Работаю')

    if (e.surface !== 'terminal') {
      return next({ ...e, props: { ...e.props, message: label } })
    }
    spinner = e.requestId
    sent = ''
    columns = Math.max(30, Math.min(90, (e.viewport?.columns ?? 80) - 4))
    const trail = list
      .slice(live?.status === 'run' ? -4 : -3, live?.status === 'run' ? -1 : undefined)
      .reverse()
      .map(s => s.label)
    const running = world.minis.filter(m => m.status === 'run').length
    const { Box, Raster, Text } = $.ui.resolve(e)

    return (
      <Box flexDirection="column">
        <Raster key="scene" columns={columns} rows={ROWS} cells={frame()} />
        <Box>
          <Text color={live?.status === 'run' ? colorOf(live) : '#d97757'} bold>
            {live?.status === 'run' ? ICON[live.kind] : '✻'} {label}
          </Text>
          {trail.length > 0 && <Text dimColor> ← {trail.join(' ← ')}</Text>}
          <Text dimColor>
            {'  '}· шаг {list.length}
            {running > 0 ? ` · помощников бегут: ${running}` : ''}
          </Text>
        </Box>
        {goalText !== '' && (
          <Text dimColor>
            <Text color={goal ? '#9ece6a' : '#ffd166'}>▶</Text> цель: {cut(goalText, columns - 8)}
          </Text>
        )}
      </Box>
    )
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Text } = $.ui.resolve(e)
    const all = await read($, session)
    const spawned = await read($, helpers)
    const room = Math.max(3, (e.viewport?.rows ?? 30) - 12)
    const files = new Map<string, { read: number; edit: number }>()
    for (const s of all) {
      if (s.file === null) continue
      const f = files.get(s.file) ?? { read: 0, edit: 0 }
      if (s.kind === 'read') f.read += 1
      else f.edit += 1
      files.set(s.file, f)
    }
    const top = [...files].sort((a, b) => b[1].edit - a[1].edit || b[1].read - a[1].read).slice(0, 8)

    return (
      <Box flexDirection="column">
        <Text color="#d97757" bold>
          {' ▐▛███▜▌  '}шагов {all.length} · ошибок {all.filter(s => s.status === 'err').length} · помощников запускал {spawned}
        </Text>
        {all.length === 0 && <Text dimColor>Шагов пока нет.</Text>}
        {all.slice(-room).map((s, i, shown) => (
          <Text>
            <Text dimColor>{String(all.length - shown.length + i + 1).padStart(3)} </Text>
            <Text color={colorOf(s)}>{s.status === 'err' ? '✕' : ICON[s.kind]} </Text>
            <Text>{s.label}</Text>
          </Text>
        ))}
        {top.length > 0 && <Text bold>Файлы</Text>}
        {top.map(([file, f]) => (
          <Text>
            <Text color={hex(COLOR.edit)}>{f.edit > 0 ? `✎${f.edit} ` : '   '}</Text>
            <Text color={hex(COLOR.read)}>{f.read > 0 ? `◉${f.read} ` : '   '}</Text>
            <Text dimColor>{file.replace(/^\/home\/[^/]+/, '~')}</Text>
          </Text>
        ))}
      </Box>
    )
  })
}
