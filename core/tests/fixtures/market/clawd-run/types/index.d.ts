export type Kind = 'read' | 'edit' | 'write' | 'bash' | 'search' | 'web' | 'agent' | 'mcp' | 'other'

export type Step = {
  id: string
  kind: Kind
  label: string
  file: string | null
  status: 'run' | 'ok' | 'err'
  at: number
}

declare module 'claude-code' {
  interface PluginState {
    'clawd-run': { turn: Step[]; session: Step[]; startedAt: number; helpers: number }
  }
}
