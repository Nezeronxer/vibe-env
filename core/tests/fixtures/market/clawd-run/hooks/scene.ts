import type { Kind, Step } from '../types'

// Мультик-история «путь к цели»: Clawd (маскот Claude Code, по официальной иконке) выбегает из домика и бежит
// к флажку с целью хода. Каждый шаг - станция на дороге, с каждой он ближе к флажку;
// пишет ответ - добежал и прыгает от радости. Движение на физике: пружина с инерцией,
// гравитация, отскоки частиц. Субагенты - клавдики гуськом за ним.
// Анимация по образцам модов cc-clawd, claude-clawd и clawdy: дыхание цветом, взгляд, моргание веками,
// присед перед прыжком, растяжение в полёте и сплющивание при приземлении, реквизит под каждый инструмент.
const DEFAULT = 0x01000000
const BODY = 0xd97757 // фирменный оранжевый Claude
const BODY_HI = 0xeb8a69
const BODY_SLEEP = 0x9c5a44
const LID = 0xa8543c // закрытые глаза - веки темнее тела, лицо остаётся лицом
const BLUSH = 0xf28bb3
const HURT = 0xff5555
const TRACK = 0x4a3a3a
const SPARK = 0xa38b8a
const ROOF = 0xb5523b
const WALL = 0xe8d5b0
const DOOR = 0x7a4a2a
const LIT = 0xffd166
const WIN = 0x9ece6a
const DARK_WINDOW = 0x5a6b7a
const SMOKE = 0x8a8a8a
const CLOUD = 0x9aa5b1
const SUN = 0xffd166
const MOON = 0xdfe6ee
const PAPER = 0xf1ead8
const INK = 0x3a3a3a
const GRAY = 0x8b929c
const SCREEN = 0x1b2420
const GLOW = 0x2f4a3c
const CODE = 0x5cc27a
const SWEAT = 0x5cc8e8
const CAP = 0x6f9cf5
const CONFETTI = [0xffd166, 0x9ece6a, 0x7aa2f7, 0xf7768e, 0xbb9af7]

// Герой - Clawd по официальной иконке Claude Code (thesvg.org/icon/claude-code): тело 7 пикселей,
// руки по бокам, глаза-прорези, четыре ноги. Рисуется по позе на пиксельном холсте полублоками
export const HERO_W = 9
// Субагент - клавдик в одну строку
const MINI = ['▝▛▜▘', '▗▛▜▖']
export const MINI_W = 4

export const ROWS = 6
const GROUND = 5 // строка дорожки
const FEET = GROUND * 2 - 1 // пиксель, на котором стоят ноги
const HOUSE_W = 8
export const DOOR_X = -6 // внутри домика: оттуда выбегают и туда возвращаются
const START = HOUSE_W + 1
const DECAY = 0.85 // каждый шаг съедает 15% оставшейся дороги

// Физика: клетки и секунды
const K = 9 // жёсткость пружины к цели
const DAMP = 5 // трение; меньше 2√K - лёгкий занос мимо цели
const VMAX = 14
const G = 26
const JUMP = 7.5 // прыжок через станцию: ~1 клетка
const CHEER = 9 // прыжок радости у цели: ~1,5 клетки
const CROUCH = 0.12 // присед перед прыжком радости
const SQUASH = 0.14 // сплющен после жёсткого приземления
const SLEEP_AFTER = 30 // секунд без дела - засыпает

export const goalX = (columns: number) => columns - 3

/** Где стоит Clawd после n шагов: всё ближе к флажку, но не доходит, пока ход не кончен */
export const heroX = (columns: number, n: number) =>
  n === 0 ? DOOR_X : START + (goalX(columns) - HERO_W - 3 - START) * (1 - DECAY ** n)

/** Станция шага i - под передней ногой Clawd, когда он там стоит */
export const stationX = (columns: number, i: number) => Math.round(heroX(columns, i + 1)) + 2

export type Mini = { id: string; x: number; status: 'run' | 'ok' | 'err' }
export type Particle = { x: number; y: number; vx: number; vy: number; g: number; life: number; ch: string; fg: number }
export type Hero = {
  x: number; vx: number; y: number; vy: number; hurt: number; errs: number
  crouch: number; spring: number; squash: number; still: number
}
export type World = { hero: Hero; parts: Particle[]; minis: Mini[] }

export const newWorld = (minis: Mini[] = []): World => ({
  hero: { x: DOOR_X, vx: 0, y: 0, vy: 0, hurt: 0, errs: 0, crouch: 0, spring: 0, squash: 0, still: 0 },
  parts: [],
  minis,
})

export type Scene = { columns: number; steps: Step[]; goal: boolean; busy: boolean }

const pick = <T,>(list: T[], rnd: () => number) => list[Math.floor(rnd() * list.length)] as T

/** Смесь двух цветов 0xRRGGBB, k от 0 до 1 */
export const mix = (a: number, b: number, k: number) => {
  const ch = (s: number) => Math.round(((a >> s) & 255) + (((b >> s) & 255) - ((a >> s) & 255)) * k)
  return (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

const dust = (w: World, x: number, dir: number, rnd: () => number) =>
  w.parts.push({ x, y: 0.2, vx: dir * (1 + rnd() * 2), vy: 1 + rnd() * 2, g: 10, life: 0.5, ch: '·', fg: SPARK })

const burst = (w: World, x: number, y: number, n: number, colors: number[], rnd: () => number) => {
  for (let i = 0; i < n; i++)
    w.parts.push({ x, y, vx: (rnd() - 0.5) * 8, vy: 3 + rnd() * 4, g: 14, life: 1.6, ch: pick(['*', '✦', '•', '·'], rnd), fg: pick(colors, rnd) })
}

/** Один шаг физики на dt секунд; мир меняется на месте */
export function simulate(w: World, dt: number, { columns, steps, goal, busy }: Scene, rnd = Math.random) {
  const h = w.hero
  const gx = goalX(columns)
  const target = goal ? gx - HERO_W - 1 : heroX(columns, steps.length)
  // ошибка: отбрасывает назад, голова кругом, вокруг неё звёздочки
  const errs = steps.filter(s => s.status === 'err').length
  if (errs > h.errs) {
    h.hurt = 1.5
    h.vx = -9
    h.vy = 6
    h.crouch = 0
    h.spring = 0
    burst(w, h.x + 4, h.y + 1.5, 4, [HURT], rnd)
  }
  h.errs = errs
  h.hurt = Math.max(0, h.hurt - dt)
  h.squash = Math.max(0, h.squash - dt)

  const onGround = h.y <= 0 && h.vy <= 0

  // бег: пружина к цели с трением - разгон, торможение, лёгкий занос; в приседе стоит на месте
  const ax = h.crouch > 0 ? -DAMP * 3 * h.vx : K * (target - h.x) - DAMP * h.vx
  const prev = h.x
  h.vx = Math.max(-VMAX, Math.min(VMAX, h.vx + ax * dt))
  h.x += h.vx * dt
  if (onGround && Math.abs(h.vx) > 4 && ax * h.vx < 0 && rnd() < 0.5) dust(w, h.x + 4, -Math.sign(h.vx), rnd)
  // на бегу пыль из-под ног, на большой скорости - полоски ветра за спиной
  if (onGround && Math.abs(h.vx) > 3 && rnd() < dt * 5) dust(w, h.x + (h.vx > 0 ? 1 : HERO_W - 2), -Math.sign(h.vx) * 0.5, rnd)
  if (Math.abs(h.vx) > 8 && rnd() < dt * 12)
    w.parts.push({ x: h.vx > 0 ? h.x - 1 : h.x + HERO_W, y: 0.8 + rnd() * 2, vx: -Math.sign(h.vx) * 3, vy: 0, g: 0, life: 0.25, ch: '─', fg: TRACK })

  // подпрыгивает на каждой станции, мимо которой бежит вперёд
  if (onGround && h.vx > 0)
    for (let i = 0; i < steps.length; i++) {
      const sx = stationX(columns, i)
      if (prev + 2 < sx && sx <= h.x + 2) h.vy = JUMP
    }

  // добежал до цели: присед, прыжок радости, флажок салютует конфетти
  if (goal && onGround && h.vy <= 0 && h.crouch <= 0 && h.spring === 0 && Math.abs(target - h.x) < 0.7) {
    h.crouch = CROUCH
    h.spring = CHEER
  }
  if (h.crouch > 0) {
    h.crouch -= dt
    if (h.crouch <= 0 && h.spring > 0) {
      h.vy = h.spring
      h.spring = 0
      burst(w, gx + 1, 3, 6, CONFETTI, rnd)
    }
  }

  // гравитация; жёсткое приземление сплющивает и поднимает пыль с обеих сторон
  if (h.y > 0 || h.vy > 0) {
    h.vy -= G * dt
    h.y += h.vy * dt
    if (h.y <= 0) {
      if (h.vy < -5) {
        dust(w, h.x + 1, -1, rnd)
        dust(w, h.x + HERO_W - 2, 1, rnd)
      }
      if (h.vy < -3) h.squash = SQUASH
      h.y = 0
      h.vy = 0
    }
  }
  h.still = onGround && Math.abs(h.vx) < 0.6 ? h.still + dt : 0

  // дым из трубы уносит ветром вправо
  if (busy && rnd() < 0.3) w.parts.push({ x: 5.5, y: 3.6, vx: 1.5 + rnd(), vy: 0.1, g: -0.25, life: 2.4, ch: '∘', fg: SMOKE })

  // частицы: гравитация, отскок от земли с потерей энергии
  for (const p of w.parts) {
    p.vy -= p.g * dt
    p.x += p.vx * dt
    p.y += p.vy * dt
    p.life -= dt
    if (p.y < 0 && p.g > 0) {
      p.y = 0
      p.vy = -p.vy * 0.45
      p.vx *= 0.6
    }
  }
  w.parts = w.parts.filter(p => p.life > 0 && p.x > -1 && p.x < columns && p.y < ROWS)

  // бегущие клавдики держатся за Clawd гуськом, закончившие идут к двери и там исчезают
  let place = 0
  w.minis = w.minis
    .map(m => {
      if (m.status !== 'run') return { ...m, x: approach(m.x, 0) }
      place += 1
      return { ...m, x: approach(m.x, Math.max(0, h.x - place * (MINI_W + 1)), 1.5) }
    })
    .filter(m => m.status === 'run' || m.x > 0)
}

type Dot = (x: number, y: number, color: number) => void
type Eyes = 'front' | 'left' | 'right' | 'up' | 'shut' | 'dizzy1' | 'dizzy2'
type Arm = 'out' | 'up' | 'down' | 'reach' | 'chin'
export type Pose = {
  shape: 'normal' | 'squash' | 'stretch'
  eyes: Eyes
  armL: Arm
  armR: Arm
  legs: 'stand' | 'runA' | 'runB' | 'tuck'
  color: number
  blush?: boolean
  cap?: boolean
}

const LEGS: Record<Pose['legs'], number[][]> = {
  stand: [[2, 3, 5, 6]],
  runA: [[1, 2, 5, 6]], // задние ноги отталкиваются
  runB: [[2, 3, 6, 7]], // передние выносятся вперёд
  tuck: [], // в воздухе ноги поджаты
}

/** Clawd в позе: x - левый край, feet - пиксель под ногами. Возвращает верхний пиксель и строку рук */
export function drawClawd(dot: Dot, x: number, feet: number, p: Pose) {
  const squash = p.shape === 'squash'
  const bodyH = squash ? 3 : p.shape === 'stretch' ? 5 : 4
  const legs = squash ? [[2, 3, 5, 6]] : LEGS[p.legs]
  const top = feet - legs.length - bodyH + 1
  const l = squash ? 0 : 1
  const r = squash ? 8 : 7 // сплющенный шире на пиксель с каждой стороны
  const eyeRow = top + 1
  const cols = p.eyes === 'right' ? [3, 6] : p.eyes === 'left' ? [2, 5] : [2, 6]
  const eyes: number[][] =
    p.eyes === 'up' ? cols.map(c => [c, eyeRow - 1])
    : p.eyes === 'dizzy1' ? [[2, eyeRow - 1], [6, eyeRow]]
    : p.eyes === 'dizzy2' ? [[2, eyeRow], [6, eyeRow - 1]]
    : cols.map(c => [c, eyeRow])
  const hole = (c: number, y: number) => eyes.some(([ec, ey]) => ec === c && ey === y)
  for (let y = top; y < top + bodyH; y++)
    for (let c = l; c <= r; c++) {
      if (!hole(c, y)) dot(x + c, y, p.color)
      else if (p.eyes === 'shut') dot(x + c, y, LID)
    }
  if (p.blush && !squash) {
    dot(x + 1, eyeRow + 1, BLUSH)
    dot(x + 7, eyeRow + 1, BLUSH)
  }
  const arm = top + bodyH - 2
  const draw = (pose: Arm, side: -1 | 1) => {
    const c = side < 0 ? l - 1 : r + 1
    const pts =
      pose === 'up' ? [[c, arm - 1], [c + side, arm - 2]]
      : pose === 'down' ? [[c, arm + 1], [c, arm + 2]]
      : pose === 'reach' ? [[c, arm], [c + side, arm]]
      : pose === 'chin' ? [[c, arm], [c, arm - 1]]
      : [[c, arm], [c, arm + 1]]
    for (const [px, py] of pts) dot(x + (px ?? 0), py ?? 0, p.color)
  }
  draw(p.armL, -1)
  draw(p.armR, 1)
  legs.forEach((row, i) => row.forEach(c => dot(x + c, top + bodyH + i, p.color)))
  // ночной колпак: белая полоска на голове, синий колпак свисает влево, на кончике помпон
  if (p.cap) {
    for (let c = l + 1; c < r; c++) dot(x + c, top - 1, WALL)
    for (let c = l + 2; c < r - 2; c++) dot(x + c, top - 2, CAP)
    dot(x + l, top - 1, CAP)
    dot(x + l - 1, top, WALL)
  }
  return { top, arm }
}

/** Что пишет ноутбук на экране, по виду шага */
const TYPED: Partial<Record<Kind, string>> = { bash: '$ ls_', edit: '+-~ ', write: '+++ ', mcp: '<=> ' }
const GLOBE = ['◐', '◓', '◑', '◒']

export type Frame = Scene & { world: World; t: number; isThinking: boolean; hour?: number; elapsed?: number }

/** Секунды хода как 1:05 */
export const clock = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

/** Одна картинка сцены как [codePoint, fg, bg] на клетку */
export function paint({ columns, steps, goal, world, t, isThinking, hour = 12, elapsed }: Frame): Uint32Array {
  const tick = Math.floor(t / 100)
  const cells = new Uint32Array(columns * ROWS * 3)
  const put = (col: number, row: number, ch: string, fg: number, bg = DEFAULT) => {
    col = Math.round(col)
    row = Math.round(row)
    if (col < 0 || col >= columns || row < 0 || row >= ROWS) return
    const i = (row * columns + col) * 3
    cells[i] = ch.codePointAt(0) ?? 32
    cells[i + 1] = fg
    cells[i + 2] = bg
  }
  const text = (col: number, row: number, line: string, fg: number) =>
    [...line].forEach((ch, c) => { if (ch !== ' ') put(col + c, row, ch, fg) })
  // пиксельный холст: две строки пикселей на клетку, рисуется полублоками в два цвета
  const pix = new Int32Array(columns * ROWS * 2).fill(-1)
  const dot: Dot = (x, y, color) => {
    x = Math.round(x)
    y = Math.round(y)
    if (x >= 0 && x < columns && y >= 0 && y < ROWS * 2) pix[y * columns + x] = color
  }
  const flush = () => {
    for (let r = 0; r < ROWS; r++)
      for (let c = 0; c < columns; c++) {
        const top = pix[2 * r * columns + c] ?? -1
        const bottom = pix[(2 * r + 1) * columns + c] ?? -1
        if (top >= 0 && bottom >= 0) put(c, r, top === bottom ? '█' : '▀', top, top === bottom ? DEFAULT : bottom)
        else if (top >= 0) put(c, r, '▀', top)
        else if (bottom >= 0) put(c, r, '▄', bottom)
      }
    pix.fill(-1)
  }
  for (let r = 0; r < ROWS; r++) for (let c = 0; c < columns; c++) put(c, r, ' ', DEFAULT)

  // небо: днём солнце, ночью луна и звёзды; облака плывут влево
  const isNight = hour < 7 || hour >= 20
  put(columns - 3, 0, '●', isNight ? MOON : SUN)
  if (isNight) for (let c = 0; c < columns - 4; c++) if ((c * 13 + 5) % 17 === 0) put(c, (c % 3 === 0 ? 1 : 0), (tick + c) % 20 === 0 ? '✦' : '·', SPARK)
  for (let k = 0; k < 3; k++) {
    const span = columns + 8
    const cx = span - 1 - ((Math.floor(tick / 3) + k * Math.floor(span / 3)) % span)
    text(cx - 4, k === 1 ? 1 : 0, k === 1 ? '▂▄▆▄▂' : '▁▃▅▃▁', CLOUD)
  }

  // дорога до флажка: пройденная часть цветом шагов, остаток тусклый
  const gx = goalX(columns)
  for (let c = 0; c <= gx; c++) put(c, GROUND, c < HOUSE_W - 1 ? '▔' : '━', TRACK)
  steps.forEach((s, i) => {
    const sx = stationX(columns, i)
    const color = s.status === 'err' ? ERR : COLOR[s.kind]
    for (let c = i === 0 ? HOUSE_W - 1 : stationX(columns, i - 1) + 1; c <= sx; c++) put(c, GROUND, '━', color)
    const blink = s.status === 'run' && tick % 4 < 2
    put(sx, GROUND, s.status === 'err' ? '✕' : blink ? '○' : ICON[s.kind], color)
  })

  // флажок цели: полотнище вьётся волной, у добежавшего Clawd зеленеет
  const h = world.hero
  const reached = goal && Math.abs(h.x - (gx - HERO_W - 1)) < 1.5
  for (let r = 2; r < GROUND; r++) put(gx, r, '┃', WALL)
  for (let c = 1; c <= 2; c++) {
    const wave = Math.round(Math.sin(t / 180 - c * 1.3) * 0.7)
    const cloth = reached ? WIN : LIT
    for (let y = 4; y <= 6; y++) dot(gx + c, y + (c === 2 ? wave : 0), (y + c) % 2 ? cloth : mix(cloth, 0xffffff, 0.3))
  }

  // субагенты: на бегу подпрыгивают через кадр
  world.minis.forEach((m, i) => {
    const color = m.status === 'err' ? HURT : m.status === 'ok' ? SPARK : BODY
    text(Math.round(m.x), GROUND - 1, MINI[(tick + i) % 2] ?? '', color)
  })

  // Clawd: поза из того, что он сейчас делает
  const live = steps.at(-1)
  const working = live?.status === 'run' ? live.kind : null
  const airborne = h.y > 0.15 || h.vy > 0
  const running = !airborne && Math.abs(h.vx) > 1.2
  const hurt = h.hurt > 0
  const cheer = reached && h.y > 0.3
  const atWork = working !== null && !airborne && !running && !hurt
  const asleep = !working && !isThinking && !goal && !hurt && h.still > SLEEP_AFTER
  const stride = Math.max(90, 240 - Math.abs(h.vx) * 12) // чем быстрее бежит, тем чаще шаги
  const step = Math.floor(t / stride) % 2
  const pose: Pose = {
    shape: h.squash > 0 || h.crouch > 0 ? 'squash' : airborne && h.vy > 2 ? 'stretch' : 'normal',
    eyes: h.vx < -1 ? 'left' : h.vx > 1 ? 'right' : 'front',
    armL: 'out',
    armR: 'out',
    legs: airborne ? 'tuck' : running ? (step ? 'runA' : 'runB') : 'stand',
    // дышит: цвет тела медленно светлеет и темнеет
    color: mix(BODY, BODY_HI, ((Math.sin(t / 650) + 1) / 2) * 0.6),
    cap: isNight,
  }
  // ошибка: голова кругом, мигает красным
  if (hurt) {
    pose.eyes = Math.floor(t / 110) % 2 ? 'dizzy1' : 'dizzy2'
    pose.armL = pose.armR = Math.floor(t / 160) % 2 ? 'up' : 'out'
    if (tick % 2 === 0) pose.color = HURT
  } else if (reached) {
    // радость у флажка: руки по очереди вверх, щёчки, в прыжке обе руки вверх
    pose.eyes = 'up'
    pose.blush = true
    const wave = Math.floor(t / 190) % 2
    pose.armL = cheer || wave ? 'up' : 'out'
    pose.armR = cheer || !wave ? 'up' : 'out'
  } else if (atWork) {
    pose.eyes = 'right'
    pose.armR = 'reach'
  } else if (isThinking && !airborne && !running) {
    // думает: рука у подбородка, глаза вверх, изредка косится на дорогу
    pose.eyes = t % 3000 < 2400 ? 'up' : 'right'
    pose.armR = 'chin'
  } else if (asleep) {
    pose.eyes = 'shut'
    pose.color = BODY_SLEEP
    pose.armL = pose.armR = t % 4000 < 2000 ? 'out' : 'down'
  } else if (!airborne && !running && h.still > 2) {
    // ждёт: оглядывается, переминается, потягивается
    const c = (t + 4000) % 12000
    pose.eyes = c < 3000 ? 'front' : c < 4200 ? 'left' : c < 7000 ? 'front' : c < 8200 ? 'right' : 'front'
    if (c > 9000 && c < 9800) pose.legs = Math.floor(t / 140) % 2 ? 'runB' : 'stand'
    if (c > 10400 && c < 11200) pose.armL = pose.armR = 'up'
  }
  // моргает раз в ~4 секунды, иногда дважды подряд
  const blink = t % 4100 < 130 || (t % 12300 > 300 && t % 12300 < 420)
  if (blink && !hurt && pose.eyes !== 'shut' && !cheer) pose.eyes = 'shut'

  const hx = Math.round(h.x)
  const bob = running && step ? 1 : 0 // на бегу тело подпрыгивает на полклетки
  const feet = FEET - Math.round(h.y * 2) - bob
  const hero: Dot = (x, y, color) => { if (x >= HOUSE_W) dot(x, y, color) } // дома, за стеной, его не видно
  const shake = hurt && h.hurt > 1.1 ? tick % 2 : 0 // первые мгновения после удара трясёт
  const { top, arm } = drawClawd(() => {}, hx, feet, pose) // реквизиту нужны строки рук и головы
  const headRow = Math.floor((top - 1) / 2) // клетка над головой

  // реквизит под шаг: книга, лист, ноутбук, лупа, глобус, свисток помощникам
  const px = hx + HERO_W + 1
  const glyphs: Array<() => void> = []
  if (atWork && working) {
    if (working === 'read') {
      // раскрытая книга в руке, глаза бегают по строкам, страницы перелистываются
      pose.eyes = Math.floor(t / 450) % 2 ? 'right' : 'front'
      for (let c = 0; c < 5; c++) for (let y = arm - 2; y <= arm; y++) dot(px - 1 + c, y, c === 2 ? mix(PAPER, INK, 0.5) : PAPER)
      const line = Math.floor(t / 300) % 4
      dot(px - 1 + (line < 2 ? line : line + 1), arm - 1, INK)
      const flip = t % 2400
      if (flip < 300) dot(px + 3 - Math.floor(flip / 100), arm - 2, mix(PAPER, 0xffffff, 0.8))
    } else if (working === 'edit' || working === 'write' || working === 'bash' || working === 'mcp') {
      // ноутбук на земле: клавиатура пикселями, экран печатает, по экрану бегает блик
      for (let c = -1; c < 7; c++) dot(px + c, FEET, GRAY)
      pose.armR = Math.floor(t / 120) % 2 ? 'reach' : 'out'
      const typed = TYPED[working] ?? '... '
      const n = 1 + (Math.floor(t / 170) % typed.length)
      const sweep = ((t / 1800) % 1) * 8 - 1
      glyphs.push(() => {
        for (let i = 0; i < 6; i++) {
          const bg = mix(SCREEN, GLOW, Math.max(0, 1 - Math.abs(i - sweep) / 1.2) * 0.9)
          const ch = i > 0 && i - 1 < n ? (typed[i - 1] ?? ' ') : i - 1 === n && Math.floor(t / 350) % 2 ? '▏' : ' '
          put(px + i, GROUND - 2, ch, i === 1 ? COLOR[working] : CODE, bg)
        }
        put(px - 1, GROUND - 2, '▐', GRAY)
        put(px + 6, GROUND - 2, '▌', GRAY)
      })
    } else if (working === 'search') {
      // лупа водит туда-сюда, глаза за ней
      const sx = px + 1 + Math.round(Math.sin(t / 400) * 2)
      pose.eyes = Math.sin(t / 400) > 0 ? 'right' : 'front'
      for (const [dx, dy] of [[0, -1], [1, -1], [-1, 0], [2, 0], [0, 1], [1, 1]]) dot(sx + (dx ?? 0), arm - 1 + (dy ?? 0), GRAY)
      if (Math.floor(t / 500) % 3 === 0) dot(sx, arm - 1, 0xffffff)
      dot(sx - 1, arm + 1, DOOR)
    } else if (working === 'web') {
      // крутит глобус над головой
      pose.armR = 'up'
      pose.eyes = 'up'
      glyphs.push(() => put(px - 1, headRow + 1, GLOBE[Math.floor(t / 200) % 4] ?? '◐', COLOR.web))
    } else if (working === 'agent') {
      // машет помощникам
      pose.armR = Math.floor(t / 250) % 2 ? 'up' : 'out'
      if (Math.floor(t / 250) % 2) glyphs.push(() => put(px, headRow + 1, '!', COLOR.agent))
    }
  }
  drawClawd(hero, hx + shake, feet, pose)
  flush()

  // домик поверх бегунов: дома их не видно; дверь открыта, пока кто-то выходит
  const leaving = (h.x > DOOR_X + 0.3 && h.x < HOUSE_W) || world.minis.some(m => m.x > 0.3 && m.x < HOUSE_W - 2)
  const home = h.x < DOOR_X + 1 && world.minis.length === 0
  text(5, 1, '▗▖', ROOF)
  text(0, 2, '▗▟████▙▖', ROOF)
  text(0, 3, ' █', WALL); text(4, 3, '██', WALL); put(6, 3, '█', WALL)
  text(0, 4, ' █', WALL); text(4, 4, '███', WALL)
  for (const r of [3, 4]) {
    put(2, r, leaving ? ' ' : '▐', DOOR)
    put(3, r, leaving ? '▏' : '▌', DOOR)
  }
  put(5, 3, '▪', home || isNight ? LIT : DARK_WINDOW, WALL)

  // ход идёт, а Clawd ещё дома: ломится в дверь - стучит, выглядывает, дверь захлопывается
  if (h.x < DOOR_X + 1 && !goal) {
    const c = t % 3600
    if (c < 1500) {
      // четыре удара: дверь вздрагивает светлее, над ней выскакивает «тук»
      const beat = c % 375
      if (beat < 140) {
        for (const r of [3, 4]) { put(2, r, '▐', mix(DOOR, LIT, 0.5)); put(3, r, '▌', mix(DOOR, LIT, 0.5)) }
        put(1, 3, '‹', SPARK)
      }
      if (beat < 260) text(0, 1, Math.floor(c / 375) % 2 ? 'тук' : 'бум', mix(LIT, TRACK, beat / 260))
    } else if (c < 2700) {
      // дверь приоткрылась: в щели оранжевый бок и глаз, косится то на дорогу, то на флажок
      const look = c < 2100 ? '▖' : '▗'
      put(2, 3, '▐', BODY); put(3, 3, look, INK, BODY)
      put(2, 4, '▐', BODY); put(3, 4, '▏', DOOR)
      if (c > 1900 && c < 2500) put(4, 2, '?', LIT)
    } else if (c < 2950) {
      // захлопнулась: пыль из-под двери
      put(1, 4, '·', SPARK); put(4, GROUND, '·', SPARK)
      put(2, 1, '!', HURT)
    }
  }

  for (const g of glyphs) g()
  const visible = hx + 4 >= HOUSE_W
  const head = visible ? hx : HOUSE_W - 3 // пока дома - думает над крышей
  if (hurt) {
    // звёздочки кружат вокруг головы
    for (let i = 0; i < 3; i++) {
      const a = (t / 900) * Math.PI * 2 + (i * Math.PI * 2) / 3
      const sy = Math.sin(a)
      put(head + 4 + Math.cos(a) * 4, headRow + (sy > 0 ? 0 : -0.4), sy > 0 ? '✦' : '·', LIT)
    }
  } else if (asleep) {
    // z улетают вверх и тают
    for (let i = 0; i < 2; i++) {
      const k = (t / 2600 + i / 2) % 1
      put(head + 7 + k * 3, headRow - k * 2, k < 0.5 ? 'z' : 'Z', mix(SPARK, TRACK, k))
    }
  } else if (isThinking && !airborne && !atWork) {
    // точки мысли загораются по очереди и поднимаются
    const dot3 = Math.floor(t / 300) % 3
    for (let d = 0; d < 3; d++) put(head + 7 + d, headRow - (d === dot3 ? 0.6 : 0), d === dot3 ? '•' : '·', d === dot3 ? BODY : SPARK)
  } else if (reached) {
    // ноты радости улетают вверх
    for (let i = 0; i < 2; i++) {
      const k = (t / 1600 + i / 2) % 1
      put(head + 9 + i * 2 + Math.sin(k * 6), headRow + 1 - k * 2.5, i ? '♫' : '♪', mix(WIN, TRACK, k))
    }
  }
  // после ошибки ход нервный: со лба катится капля пота
  if (h.errs > 0 && !hurt && visible && t % 2400 < 900) put(hx - 1, headRow + 1 + Math.floor((t % 2400) / 450), '▾', SWEAT)

  // часы хода прямо в кадре: тикают, даже когда строка под сценой не перерисовывается
  if (elapsed !== undefined) text(0, 0, clock(elapsed), Math.floor(t / 500) % 2 ? SPARK : TRACK)

  // частицы поверх всего: дым тает по пути, пыль и конфетти падают
  for (const p of world.parts) {
    const ch = p.fg === SMOKE ? (p.life > 1.6 ? '∘' : p.life > 0.8 ? '°' : '·') : p.ch
    put(p.x, GROUND - 1 - Math.round(p.y), ch, p.fg)
  }
  return cells
}

/** Шаг к цели не больше чем на speed клеток */
export const approach = (from: number, to: number, speed = 1) => from + Math.sign(to - from) * Math.min(speed, Math.abs(to - from))

export const COLOR: Record<Kind, number> = {
  read: 0x7aa2f7,
  edit: 0xff9e64,
  write: 0xe0af68,
  bash: 0x9ece6a,
  search: 0x7dcfff,
  web: 0xbb9af7,
  agent: 0xf7768e,
  mcp: 0x73daca,
  other: 0xc0caf5,
}
export const ERR = 0xff5555

export const ICON: Record<Kind, string> = {
  read: '◉', edit: '✎', write: '✚', bash: '▶', search: '⌕', web: '◆', agent: '★', mcp: '◈', other: '●',
}

export const toBase64 = (cells: Uint32Array) =>
  (new Uint8Array(cells.buffer) as Uint8Array & { toBase64(): string }).toBase64()

const base = (p: string) => p.split('/').filter(Boolean).at(-1) ?? p
export const cut = (s: string, n: number) => (s.length > n ? s.slice(0, n - 1) + '…' : s)

/** Что за шаг: вид, подпись по-русски, файл */
export function describe(e: Record<string, unknown>): { kind: Kind; label: string; file: string | null } {
  const tool = String(e.tool)
  const str = (k: string) => (typeof e[k] === 'string' ? (e[k] as string) : '')
  const file = str('file_path') || str('notebook_path') || null
  switch (tool) {
    case 'Read':
      return { kind: 'read', label: `Читаю ${base(file ?? '')}`, file }
    case 'Edit':
    case 'MultiEdit':
    case 'NotebookEdit':
      return { kind: 'edit', label: `Правлю ${base(file ?? '')}`, file }
    case 'Write':
      return { kind: 'write', label: `Пишу ${base(file ?? '')}`, file }
    case 'Bash':
      return { kind: 'bash', label: cut(str('description') || `$ ${str('command')}`, 60), file: null }
    case 'Grep':
    case 'Glob':
      return { kind: 'search', label: `Ищу ${cut(str('pattern'), 40)}`, file: null }
    case 'WebFetch':
    case 'WebSearch':
      return { kind: 'web', label: `В сети: ${cut(str('query') || str('url'), 50)}`, file: null }
    case 'Agent':
    case 'Task':
      return { kind: 'agent', label: `Помощник: ${cut(str('description'), 50)}`, file: null }
    case 'Skill':
      return { kind: 'other', label: `Скилл ${str('skill')}`, file: null }
  }
  if (tool.startsWith('mcp__')) {
    const [, server = '', name = ''] = tool.split('__')
    return { kind: 'mcp', label: `${server}: ${name}`, file: null }
  }
  return { kind: 'other', label: tool, file: null }
}
