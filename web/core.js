const state = {
  accounts: [],
  byUuid: new Map(),
  cards: new Map(),
  playerInfo: new Map(),
  pendingInfo: new Set(),
  firstRender: true,
  query: '',
  selected: null,
  viewer: null,
  clients: [],
  currentClient: '',
  profiles: [],
  editingProfile: null,
  editorFiles: {},
  groups: [],
  selectedGroup: null,
  groupLaunchProgress: null,
  filters: { groups: new Set(), noRole: false, expired: false },
  sort: { key: null, dir: 'desc' },
}

const grid = document.getElementById('grid')

const emptyBox = document.getElementById('empty')

const searchInput = document.getElementById('search')

const overlay = document.getElementById('overlay')

const skinStage = document.getElementById('skin-stage')

const STAR_SVG =
  '<svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3.5l2.6 5.3 5.9.9-4.2 4.1 1 5.8-5.3-2.8-5.3 2.8 1-5.8-4.2-4.1 5.9-.9z"/></svg>'

const CARET_SVG =
  '<svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg>'

const PLAY_SVG =
  '<svg viewBox="0 0 24 24" width="20" height="20"><path d="M9 6.2l9.5 5.8L9 17.8z" fill="currentColor"/></svg>'

const DOWNLOAD_SVG =
  '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12"/><path d="M7 11l5 5 5-5"/><path d="M5 21h14"/></svg>'

const PLUS_SVG =
  '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 6v12M6 12h12"/></svg>'

let promptResolve = null

let confirmResolve = null

const LS_TOGGLES = [
  { key: 'minGraphics', label: 'Минимальная графика' },
  { key: 'fullscreen', label: 'Полный экран' },
  { key: 'discordRPC', label: 'Discord RPC' },
  { key: 'autoEnter', label: 'Автовход в игру' },
  { key: 'debugMode', label: 'Режим отладки' },
]

const LS_TRISTATE = [
  { key: 'animations', label: 'Анимации' },
  { key: 'fastRender', label: 'Быстрый рендер' },
]

const launchingUuids = new Set()

const SORT_OPTIONS = [
  { key: 'launched', label: 'Недавно запущенные' },
  { key: 'added', label: 'Недавно добавленные' },
  { key: 'expiry', label: 'Срок токена' },
]

const CHECK_SVG =
  '<svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5 9-10"/></svg>'

const ARROW_SVG =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14M6 11l6-6 6 6"/></svg>'

const RESET_SVG =
  '<svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 1 3 6.7"/><path d="M3 20v-5h5"/></svg>'

let lastModalStats = ''

let lastModalGroups = ''

let skinLibPromise = null

const textureCache = new Map()

const DEFAULT_ZOOM = 0.76

const XMARK_SVG =
  '<svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M5 5l14 14M19 5L5 19"/></svg>'

const logsState = { open: false, uuid: null, session: null, timer: null, tick: null, current: null }

const labelInput = document.getElementById('modal-label')

const IMPORT_FILES = ['binds.json', 'options.txt', 'optionsof.txt', 'voicechat.json']

const importDrop = document.getElementById('import-drop')

const importInput = document.getElementById('import-file-input')

let credList = []

let credEntryIndex = -1

let importSnap = null

let importTimer = null

const credDrop = document.getElementById('cred-drop')

const credInput = document.getElementById('cred-file-input')

let importCancelling = false

const mRamSlider = document.getElementById('modal-ram-slider')

const mRamInput = document.getElementById('modal-ram-input')

const gRamSlider = document.getElementById('ram-modal-slider')

const gRamInput = document.getElementById('ram-modal-input')

const LAUNCHER_LABELS = { normal: 'Обычный лаунчер', jar: 'JAR-версия лаунчера', new: 'Новый лаунчер', custom: 'Свой лаунчер' }

let currentLauncher = 'jar'

let customLauncherPath = ''

let caps = { os: '', credentialImport: true, autoPlay: true, exeLaunchers: true, tray: true }

let currentTheme = localStorage.getItem('ac-theme') === 'light' ? 'light' : 'dark'

let statsSnap = null

let statsOff = false
