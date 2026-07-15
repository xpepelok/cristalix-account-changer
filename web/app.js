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
  filters: { groups: new Set(), noRole: false, expired: false },
  sort: { key: null, dir: 'desc' },
}

const grid = document.getElementById('grid')
const emptyBox = document.getElementById('empty')
const searchInput = document.getElementById('search')
const overlay = document.getElementById('overlay')
const skinStage = document.getElementById('skin-stage')

function skinBg(uuid) {
  return `url('/skin/${uuid}')`
}

function shortId(uuid) {
  return uuid.slice(0, 8)
}

function fmtCount(n) {
  if (n >= 1000000) {
    const v = n / 1000000
    return (Number.isInteger(v) ? v : v.toFixed(1)) + 'kk'
  }
  if (n >= 1000) {
    const v = n / 1000
    return (Number.isInteger(v) ? v : v.toFixed(1)) + 'k'
  }
  return String(n)
}

function relTime(ts) {
  if (!ts) return null
  const diff = Math.floor(Date.now() / 1000) - ts
  if (diff < 60) return 'только что'
  if (diff < 3600) return `${Math.floor(diff / 60)} мин назад`
  if (diff < 86400) return `${Math.floor(diff / 3600)} ч назад`
  return `${Math.floor(diff / 86400)} дн назад`
}

function toast(message, isError) {
  const el = document.getElementById('toast')
  el.textContent = message
  el.className = 'toast show' + (isError ? ' err' : '')
  el.hidden = false
  clearTimeout(toast._t)
  toast._t = setTimeout(() => {
    el.className = 'toast'
  }, 2600)
}

async function apiGet(path) {
  const r = await fetch(path)
  if (!r.ok) throw new Error(await r.text())
  return r.json()
}

async function apiPost(path, body) {
  const r = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  })
  const data = await r.json().catch(() => ({}))
  if (!r.ok) throw new Error(data.error || 'Ошибка')
  return data
}

function accountBadge(acc) {
  if (!acc.launchable) return { cls: 'notoken', text: 'нет токена' }
  if (state.debugExpired || acc.expired) return { cls: 'expired', text: 'истёк' }
  return { cls: 'ready', text: 'готов' }
}

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

function showMenu(menu) {
  if (!menu) return
  menu.classList.remove('dd-closing')
  menu.hidden = false
}
function hideMenuAnimated(menu) {
  if (!menu || menu.hidden) return
  menu.classList.add('dd-closing')
  setTimeout(() => {
    if (menu.classList.contains('dd-closing')) {
      menu.hidden = true
      menu.classList.remove('dd-closing')
    }
  }, 130)
}
function closeAllDropdowns() {
  document.querySelectorAll('.ac-dd.open').forEach((el) => el._close && el._close())
}
document.addEventListener('click', closeAllDropdowns)

function initDropdown(id, onChange) {
  const el = document.getElementById(id)
  el.innerHTML =
    '<button class="ac-dd-trigger" type="button"><span class="ac-dd-value"></span><span class="ac-dd-caret">' +
    CARET_SVG +
    '</span></button><div class="ac-dd-menu" hidden></div>'
  const trigger = el.querySelector('.ac-dd-trigger')
  const menu = el.querySelector('.ac-dd-menu')
  const valueEl = el.querySelector('.ac-dd-value')
  el._value = ''
  el._close = () => {
    el.classList.remove('open')
    hideMenuAnimated(menu)
  }
  trigger.addEventListener('click', (e) => {
    e.stopPropagation()
    const willOpen = !el.classList.contains('open')
    closeAllDropdowns()
    if (willOpen) {
      el.classList.add('open')
      showMenu(menu)
    }
  })
  el.set = (options, value) => {
    el._value = value
    const cur = options.find((o) => o.value === value)
    valueEl.textContent = cur ? cur.label : options[0] ? options[0].label : ''
    menu.innerHTML = options
      .map(
        (o) =>
          `<button class="ac-dd-item${o.value === value ? ' active' : ''}" data-value="${esc(o.value)}">${esc(o.label)}</button>`,
      )
      .join('')
    menu.querySelectorAll('.ac-dd-item').forEach((item) => {
      item.addEventListener('click', (e) => {
        e.stopPropagation()
        el._value = item.dataset.value
        valueEl.textContent = item.textContent
        menu.querySelectorAll('.ac-dd-item').forEach((x) => x.classList.toggle('active', x === item))
        el._close()
        if (onChange) onChange(el._value)
      })
    })
  }
  return el
}

function cardAccount(card) {
  return state.byUuid.get(card._uuid)
}

function buildCard(acc) {
  const card = document.createElement('div')
  card.className = 'acc-card'

  const avatarWrap = document.createElement('span')
  avatarWrap.className = 'avatar-wrap'
  const avatar = document.createElement('span')
  avatar.className = 'avatar'
  avatar.style.backgroundImage = skinBg(acc.uuid)
  const hat = document.createElement('span')
  hat.className = 'avatar-hat'
  hat.style.backgroundImage = skinBg(acc.uuid)
  avatar.appendChild(hat)
  const dot = document.createElement('span')
  dot.className = 'online-dot'
  avatarWrap.appendChild(avatar)
  avatarWrap.appendChild(dot)

  const body = document.createElement('div')
  body.className = 'acc-body'

  const title = document.createElement('div')
  title.className = 'acc-title'
  const pin = document.createElement('button')
  pin.className = 'pin-btn'
  pin.innerHTML = STAR_SVG
  pin.title = 'Закрепить'
  const chip = document.createElement('span')
  chip.className = 'grp-chip'
  chip.hidden = true
  const name = document.createElement('div')
  const labelTag = document.createElement('span')
  labelTag.className = 'label-tag'
  labelTag.hidden = true
  title.appendChild(pin)
  title.appendChild(chip)
  title.appendChild(name)
  title.appendChild(labelTag)

  const meta = document.createElement('div')
  meta.className = 'acc-meta'
  const badgeEl = document.createElement('span')
  const sub = document.createElement('span')
  sub.className = 'acc-sub'
  meta.appendChild(badgeEl)
  meta.appendChild(sub)

  body.appendChild(title)
  body.appendChild(meta)

  const playBtn = document.createElement('button')
  playBtn.className = 'play-btn'

  card.appendChild(avatarWrap)
  card.appendChild(body)
  card.appendChild(playBtn)

  card._name = name
  card._chip = chip
  card._badge = badgeEl
  card._sub = sub
  card._play = playBtn
  card._pin = pin
  card._dot = dot
  card._labelTag = labelTag

  card.addEventListener('contextmenu', (e) => {
    e.preventDefault()
    openCardMenu(e.clientX, e.clientY, cardAccount(card))
  })

  card.addEventListener('click', () => {
    const current = cardAccount(card)
    if (current) openModal(current)
  })
  playBtn.addEventListener('click', (e) => {
    e.stopPropagation()
    togglePlay(cardAccount(card))
  })
  pin.addEventListener('click', (e) => {
    e.stopPropagation()
    togglePin(cardAccount(card))
  })
  return card
}

async function togglePin(acc) {
  if (!acc) return
  try {
    await apiPost('/api/pin', { uuid: acc.uuid, pinned: !acc.pinned })
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
}

function closeCardMenu() {
  const m = document.getElementById('ctx-menu')
  if (m.hidden) return
  m.classList.remove('show')
  m.hidden = true
  m.innerHTML = ''
}

async function stopLaunch(acc) {
  if (!acc) return
  try {
    await apiPost('/api/stop', { uuid: acc.uuid })
    toast('Останавливаю запуск ' + (acc.name || 'аккаунта') + '…')
    setTimeout(loadAccounts, 400)
  } catch (e) {
    toast(e.message, true)
  }
}

function openCardMenu(x, y, acc) {
  if (!acc) return
  const m = document.getElementById('ctx-menu')
  const items = []
  items.push({ label: 'Информация об аккаунте', onClick: () => openModal(acc) })
  if (acc.running) items.push({ label: 'Закрыть клиент', onClick: () => togglePlay(acc) })
  else if (acc.launching) items.push({ label: 'Остановить запуск', onClick: () => stopLaunch(acc) })
  else if (acc.launchable) items.push({ label: 'Запустить', onClick: () => togglePlay(acc) })
  items.push({ label: acc.pinned ? 'Открепить' : 'Закрепить', onClick: () => togglePin(acc) })
  const g = activeGroup()
  if (g) {
    items.push({ sep: true })
    items.push({
      label: 'Убрать из «' + g.name + '»',
      danger: true,
      onClick: async () => {
        try {
          await apiPost('/api/groups/members', { id: g.id, remove: acc.uuid })
          await loadGroups()
        } catch (e) {
          toast(e.message, true)
        }
      },
    })
  }
  items.push({ sep: true })
  items.push({
    label: 'Удалить аккаунт',
    danger: true,
    onClick: async () => {
      const ok = await confirmDialog('Удалить аккаунт?', 'Аккаунт «' + esc(acc.name || shortId(acc.uuid)) + '» будет убран из списка.')
      if (!ok) return
      try {
        await apiPost('/api/forget', { uuid: acc.uuid })
        loadAccounts()
        loadGroups()
      } catch (e) {
        toast(e.message, true)
      }
    },
  })

  m.innerHTML = items
    .map((it) =>
      it.sep ? '<div class="ctx-sep"></div>' : '<button class="ctx-item' + (it.danger ? ' danger' : '') + '">' + esc(it.label) + '</button>',
    )
    .join('')
  const handlers = items.filter((it) => !it.sep)
  m.querySelectorAll('.ctx-item').forEach((el, i) => {
    el.addEventListener('click', () => {
      closeCardMenu()
      handlers[i].onClick()
    })
  })

  m.classList.remove('show')
  m.hidden = false
  m.style.left = '0px'
  m.style.top = '0px'
  const rect = m.getBoundingClientRect()
  let px = x
  let py = y
  if (px + rect.width > window.innerWidth - 8) px = window.innerWidth - rect.width - 8
  if (py + rect.height > window.innerHeight - 8) py = window.innerHeight - rect.height - 8
  m.style.left = Math.max(8, px) + 'px'
  m.style.top = Math.max(8, py) + 'px'
  void m.offsetWidth
  m.classList.add('show')
}

function fillCard(card, acc) {
  card._uuid = acc.uuid
  if (acc.name) {
    card._name.className = 'acc-name'
    card._name.textContent = acc.name
  } else {
    card._name.className = 'acc-name unknown'
    card._name.textContent = shortId(acc.uuid) + '…'
  }
  card._pin.classList.toggle('active', !!acc.pinned)
  if (acc.label) {
    card._labelTag.hidden = false
    card._labelTag.textContent = acc.label
  } else {
    card._labelTag.hidden = true
  }

  const badge = accountBadge(acc)
  card._badge.className = 'badge ' + badge.cls
  card._badge.textContent = badge.text
  card._sub.textContent = relTime(acc.lastLaunched) || ''

  if (acc.running) {
    card._play.className = 'play-btn running'
    card._play.innerHTML = '<span class="glyph-stop"></span>'
    card._play.title = 'Закрыть аккаунт'
  } else if (acc.launching) {
    card._play.className = 'play-btn launching'
    card._play.innerHTML = '<span class="glyph-spin"></span>'
    card._play.title = 'Запускается…'
  } else {
    card._play.className = 'play-btn'
    card._play.innerHTML = PLAY_SVG
    card._play.title = acc.launchable ? 'Запуск' : 'Нет токена'
  }

  applyPlayerInfo(card, acc)
  if (acc.name) ensurePlayerInfo(acc.name)
}

function primaryGroup(info) {
  if (!info) return null
  if (info.staff && info.staff !== 'PLAYER') {
    return { label: info.label || info.staff, color: info.color || '#8c8c8c' }
  }
  if (info.donate && info.donate !== 'NO') {
    return { label: info.donate, color: info.donateColor || info.color || '#e3a400' }
  }
  return null
}

function applyPlayerInfo(card, acc) {
  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  const grp = primaryGroup(info)
  if (grp) {
    card._chip.hidden = false
    card._chip.textContent = grp.label
    card._chip.style.setProperty('--chip', grp.color)
    card.style.setProperty('--grp', grp.color)
  } else {
    card._chip.hidden = true
    card.style.removeProperty('--grp')
  }
  const online = info ? info.online : ''
  card._dot.className = 'online-dot' + (online === 'online' ? ' on' : online === 'offline' ? ' off' : '')
  card._dot.title = online === 'online' ? 'В сети' : online === 'offline' ? 'Не в сети' : ''
}

async function ensurePlayerInfo(name) {
  const key = name.toLowerCase()
  if (state.playerInfo.has(key) || state.pendingInfo.has(key)) return
  state.pendingInfo.add(key)
  try {
    const info = await apiGet('/api/player?name=' + encodeURIComponent(name))
    if (info && info.name) state.playerInfo.set(key, info)
    else if (!state.playerInfo.get(key)) state.playerInfo.set(key, null)
  } catch (e) {
    if (!state.playerInfo.get(key)) state.playerInfo.set(key, null)
  }
  state.pendingInfo.delete(key)
  state.cards.forEach((card) => {
    const acc = cardAccount(card)
    if (acc && acc.name && acc.name.toLowerCase() === key) applyPlayerInfo(card, acc)
  })
  if (state.selected && state.selected.name && state.selected.name.toLowerCase() === key) {
    renderModalGroups(state.selected)
    renderModalStats(state.selected)
  }
}

function openProfile(name) {
  const url = 'https://top.cristalix.gg/profile/' + encodeURIComponent(name)
  if (typeof window.acOpenUrl === 'function') {
    window.acOpenUrl(url)
  } else {
    window.open(url, '_blank')
  }
}

async function loadProfiles() {
  try {
    const data = await apiGet('/api/profiles')
    state.profiles = data.profiles || []
  } catch (e) {
    state.profiles = []
  }
}

function fillProfileSelect(acc) {
  const names = state.profiles.slice()
  const current = names.includes(acc.profile) ? acc.profile : ''
  const options = [{ value: '', label: '- не выбран -' }].concat(names.map((p) => ({ value: p, label: p })))
  state.profileDD.set(options, current)
}

// ---- Prompt ----
let promptResolve = null
function askName(title, initial) {
  return new Promise((resolve) => {
    promptResolve = resolve
    document.getElementById('prompt-title').textContent = title
    const inp = document.getElementById('prompt-input')
    inp.value = initial || ''
    document.getElementById('prompt-overlay').hidden = false
    setTimeout(() => inp.focus(), 30)
  })
}
function closePrompt(value) {
  document.getElementById('prompt-overlay').hidden = true
  if (promptResolve) {
    const r = promptResolve
    promptResolve = null
    r(value)
  }
}

let confirmResolve = null
function confirmDialog(title, text) {
  return new Promise((resolve) => {
    confirmResolve = resolve
    document.getElementById('confirm-title').textContent = title
    document.getElementById('confirm-text').innerHTML = text
    document.getElementById('confirm-overlay').hidden = false
  })
}
function closeConfirm(value) {
  document.getElementById('confirm-overlay').hidden = true
  if (confirmResolve) {
    const r = confirmResolve
    confirmResolve = null
    r(value)
  }
}

function clampRam(v) {
  return Math.max(1024, Math.min(32768, Math.round((+v || 1024) / 512) * 512))
}
function setModalRam(acc) {
  const v = acc && acc.ram >= 512 ? acc.ram : 2048
  document.getElementById('modal-ram-slider').value = v
  document.getElementById('modal-ram-input').value = v
}
async function saveModalRam(v) {
  if (!state.selected) return
  const ram = clampRam(v)
  document.getElementById('modal-ram-slider').value = ram
  document.getElementById('modal-ram-input').value = ram
  try {
    await apiPost('/api/account/ram', { uuids: [state.selected.uuid], ram })
    state.selected.ram = ram
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
}

const LS_TOGGLES = [
  { key: 'minGraphics', label: 'Минимальная графика' },
  { key: 'fullscreen', label: 'Полный экран' },
  { key: 'discordRPC', label: 'Discord RPC' },
  { key: 'autoEnter', label: 'Автовход в игру' },
  { key: 'debugMode', label: 'Режим отладки' },
]

function renderLsToggles() {
  const box = document.getElementById('ls-toggles')
  box.innerHTML = LS_TOGGLES.map(
    (t) =>
      '<div class="ls-toggle-row"><span class="ls-toggle-label">' + t.label + '</span>' +
      '<button class="toggle' + (state.lsSettings[t.key] ? ' on' : '') + '" data-key="' + t.key + '" role="switch"><span class="toggle-knob"></span></button></div>',
  ).join('')
  box.querySelectorAll('.toggle').forEach((b) => {
    b.addEventListener('click', () => {
      const k = b.dataset.key
      state.lsSettings[k] = !state.lsSettings[k]
      b.classList.toggle('on', state.lsSettings[k])
    })
  })
}

function openRamModal() {
  const group = activeGroup()
  state.ramMembers = group ? (group.members || []).map((u) => state.byUuid.get(u)).filter(Boolean) : []
  state.ramPick = new Set(state.ramMembers.map((m) => m.uuid))
  state.lsSettings = { minGraphics: false, fullscreen: false, discordRPC: false, autoEnter: false, debugMode: false }
  state.lsClient = { renderDistance: 0, maxFps: 0, animations: 0, fastRender: 0 }
  document.getElementById('ram-modal-slider').value = 2048
  document.getElementById('ram-modal-input').value = 2048
  renderLsToggles()
  renderLsClient()
  renderRamPick()
  document.getElementById('ram-overlay').hidden = false
}

const LS_TRISTATE = [
  { key: 'animations', label: 'Анимации' },
  { key: 'fastRender', label: 'Быстрый рендер' },
]

function renderLsClient() {
  const box = document.getElementById('ls-client')
  const seg = (key) => {
    const cur = state.lsClient[key]
    const opt = (v, t) => '<button class="ls-seg-btn' + (cur === v ? ' on' : '') + '" data-val="' + v + '">' + t + '</button>'
    return '<div class="ls-seg" data-key="' + key + '"><div class="ls-seg-thumb"></div>' + opt(0, 'Не менять') + opt(1, 'Выкл') + opt(2, 'Вкл') + '</div>'
  }
  box.innerHTML =
    '<div class="ls-client-row"><span class="ls-toggle-label">Прогрузка чанков</span>' +
    '<input type="number" class="ram-input ls-num" id="ls-chunks" min="2" max="32" step="1" value="' + (state.lsClient.renderDistance || '') + '"></div>' +
    '<div class="ls-client-row"><span class="ls-toggle-label">Макс. FPS</span>' +
    '<input type="number" class="ram-input ls-num" id="ls-fps" min="5" max="260" step="5" value="' + (state.lsClient.maxFps || '') + '"></div>' +
    LS_TRISTATE.map((t) => '<div class="ls-client-row"><span class="ls-toggle-label">' + t.label + '</span>' + seg(t.key) + '</div>').join('')
  document.getElementById('ls-chunks').addEventListener('change', (e) => {
    const raw = +e.target.value || 0
    const v = raw > 0 ? Math.max(2, Math.min(32, raw)) : 0
    state.lsClient.renderDistance = v
    e.target.value = v || ''
  })
  document.getElementById('ls-fps').addEventListener('change', (e) => {
    const raw = +e.target.value || 0
    const v = raw > 0 ? Math.max(5, Math.min(260, raw)) : 0
    state.lsClient.maxFps = v
    e.target.value = v || ''
  })
  box.querySelectorAll('.ls-seg').forEach((segEl) => {
    const key = segEl.dataset.key
    const thumb = segEl.querySelector('.ls-seg-thumb')
    const moveThumb = (v) => {
      const w = parseFloat(getComputedStyle(thumb).width) || 0
      thumb.style.left = 3 + v * w + 'px'
    }
    segEl.querySelectorAll('.ls-seg-btn').forEach((b) => {
      b.addEventListener('click', () => {
        const v = +b.dataset.val
        state.lsClient[key] = v
        segEl.querySelectorAll('.ls-seg-btn').forEach((x) => x.classList.toggle('on', x === b))
        moveThumb(v)
      })
    })
  })
}
function closeRamModal() {
  document.getElementById('ram-overlay').hidden = true
}
function renderRamPick() {
  const box = document.getElementById('ram-pick-list')
  if (!state.ramMembers || !state.ramMembers.length) {
    box.innerHTML = '<div class="addmembers-empty">В группе нет аккаунтов</div>'
    return
  }
  box.innerHTML = state.ramMembers
    .map((a) => {
      const on = state.ramPick.has(a.uuid)
      return (
        '<button class="ram-pick-item' + (on ? ' on' : '') + '" data-uuid="' + esc(a.uuid) + '">' +
        '<span class="filter-box">' + (on ? CHECK_SVG : '') + '</span>' +
        '<span class="ram-pick-ava head-ava" style="--skin:' + skinBg(a.uuid) + '"></span>' +
        '<span class="ram-pick-name">' + esc(a.name || shortId(a.uuid)) + '</span>' +
        '<span class="ram-pick-cur">' + (a.ram >= 512 ? a.ram + ' МБ' : 'авто') + '</span></button>'
      )
    })
    .join('')
  box.querySelectorAll('.ram-pick-item').forEach((item) => {
    item.addEventListener('click', () => {
      const u = item.dataset.uuid
      if (state.ramPick.has(u)) state.ramPick.delete(u)
      else state.ramPick.add(u)
      renderRamPick()
    })
  })
}
async function applyGroupRam() {
  const ram = clampRam(document.getElementById('ram-modal-input').value)
  const uuids = Array.from(state.ramPick || [])
  if (!uuids.length) {
    toast('Выбери хотя бы один аккаунт', true)
    return
  }
  const chunks = Math.max(0, Math.min(32, +document.getElementById('ls-chunks').value || 0))
  const fps = Math.max(0, Math.min(260, +document.getElementById('ls-fps').value || 0))
  const payload = Object.assign(
    { uuids, ram, renderDistance: chunks, maxFps: fps, animations: state.lsClient.animations, fastRender: state.lsClient.fastRender },
    state.lsSettings,
  )
  try {
    await apiPost('/api/account/launch-settings', payload)
    toast('Настройки применены · ' + uuids.length + ' акк.')
    loadAccounts()
    closeRamModal()
  } catch (e) {
    toast(e.message, true)
  }
}

function bindRamSteppers(input, slider, onChange) {
  const wrap = input.parentElement.querySelector('.ram-step-wrap')
  if (!wrap) return
  wrap.querySelectorAll('.ram-step').forEach((btn) => {
    btn.addEventListener('click', () => {
      const cur = clampRam(input.value)
      const next = clampRam(cur + (btn.dataset.step === 'up' ? 512 : -512))
      input.value = next
      if (slider) slider.value = next
      if (onChange) onChange(next)
    })
  })
}

// ---- Profiles manager ----
function openProfiles() {
  document.getElementById('profiles-overlay').hidden = false
  state.editingProfile = null
  document.getElementById('profiles-editor').innerHTML =
    '<div class="editor-empty">Выбери профиль слева, чтобы отредактировать его параметры, либо создай новый из текущих настроек игры.</div>'
  renderProfilesList()
}
function closeProfiles() {
  document.getElementById('profiles-overlay').hidden = true
}

async function renderProfilesList() {
  await loadProfiles()
  const list = document.getElementById('profiles-list')
  if (!state.profiles.length) {
    list.innerHTML = '<div class="editor-empty" style="padding:18px 8px">Профилей пока нет</div>'
    return
  }
  list.innerHTML = state.profiles
    .map(
      (p) =>
        `<button class="profile-item${p === state.editingProfile ? ' active' : ''}" data-name="${esc(p)}"><span class="profile-item-name">${esc(p)}</span></button>`,
    )
    .join('')
  list.querySelectorAll('.profile-item').forEach((el) => {
    el.addEventListener('click', () => editProfile(el.dataset.name))
  })
}

function prettyJson(text) {
  if (!text || !text.trim()) return text
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch (e) {
    return text
  }
}

function parseLines(text) {
  return (text || '').split(/\r?\n/).map((line) => {
    const i = line.indexOf(':')
    if (i > 0) return { kv: true, k: line.slice(0, i), v: line.slice(i + 1) }
    return { kv: false, raw: line }
  })
}

function buildEditor(name, content) {
  const kvFiles = ['options.txt', 'optionsof.txt']
  const jsonFiles = ['binds.json', 'voicechat.json']
  state.editorFiles = {}
  let html = `<div class="editor-scroll"><div class="editor-head"><div class="editor-name">${esc(name)}</div></div>`
  for (const f of kvFiles) {
    const lines = parseLines(content[f] || '')
    state.editorFiles[f] = lines
    html += `<div class="editor-file" data-file="${f}"><div class="editor-file-title">${f}</div><div class="kv-rows">`
    const rows = lines
      .map((l, idx) =>
        l.kv
          ? `<div class="kv-row"><span class="kv-key" title="${esc(l.k)}">${esc(l.k)}</span><input class="kv-val" data-idx="${idx}" value="${esc(l.v)}" /></div>`
          : '',
      )
      .join('')
    html += rows || '<div class="editor-empty" style="padding:8px">файл пуст</div>'
    html += `</div></div>`
  }
  for (const f of jsonFiles) {
    html += `<div class="editor-file" data-file="${f}"><div class="editor-file-title">${f}</div><textarea class="json-area" data-file="${f}" spellcheck="false">${esc(prettyJson(content[f] || ''))}</textarea></div>`
  }
  html += `</div>`
  html += `<div class="editor-actions"><button class="action-btn primary" id="editor-save">Сохранить профиль</button><button class="action-btn danger-ghost" id="editor-delete">Удалить</button></div>`
  return html
}

function highlightProfile(name) {
  document.querySelectorAll('#profiles-list .profile-item').forEach((el) => {
    el.classList.toggle('active', el.dataset.name === name)
  })
}

async function editProfile(name) {
  state.editingProfile = name
  highlightProfile(name)
  let content = {}
  try {
    content = await apiGet('/api/profiles/content?name=' + encodeURIComponent(name))
  } catch (e) {
    /* empty */
  }
  if (state.editingProfile !== name) return
  const editor = document.getElementById('profiles-editor')
  editor.innerHTML = buildEditor(name, content)
  document.getElementById('editor-save').addEventListener('click', () => saveEditorProfile(name))
  document.getElementById('editor-delete').addEventListener('click', () => deleteEditorProfile(name))
}

function collectEditor() {
  const files = {}
  document.querySelectorAll('#profiles-editor .editor-file').forEach((fileEl) => {
    const f = fileEl.dataset.file
    const ta = fileEl.querySelector('.json-area')
    if (ta) {
      files[f] = ta.value
      return
    }
    const lines = (state.editorFiles[f] || []).map((l) => ({ ...l }))
    fileEl.querySelectorAll('.kv-val').forEach((inp) => {
      const idx = +inp.dataset.idx
      if (lines[idx] && lines[idx].kv) lines[idx].v = inp.value
    })
    files[f] = lines.map((l) => (l.kv ? l.k + ':' + l.v : l.raw)).join('\n')
  })
  return files
}

async function saveEditorProfile(name) {
  const files = collectEditor()
  for (const f of ['binds.json', 'voicechat.json']) {
    const val = files[f]
    if (val !== undefined && val.trim()) {
      try {
        JSON.parse(val)
      } catch (e) {
        const ta = document.querySelector(`.json-area[data-file="${f}"]`)
        if (ta) ta.classList.add('bad')
        toast('Некорректный JSON в ' + f, true)
        return
      }
    }
  }
  try {
    await apiPost('/api/profiles/update', { name, files })
    toast('Профиль сохранён')
  } catch (e) {
    toast(e.message, true)
  }
}

async function deleteEditorProfile(name) {
  const ok = await confirmDialog('Удалить профиль?', 'Вы уверены, что хотите удалить профиль «' + esc(name) + '»?')
  if (!ok) return
  try {
    await apiPost('/api/profiles/delete', { name })
    toast('Профиль удалён')
    state.editingProfile = null
    document.getElementById('profiles-editor').innerHTML =
      '<div class="editor-empty">Выбери профиль слева или создай новый.</div>'
    renderProfilesList()
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
}

const launchingUuids = new Set()
async function togglePlay(acc, client) {
  if (!acc) return
  if (acc.launching) return
  if (acc.running) {
    try {
      await apiPost('/api/stop', { uuid: acc.uuid })
      toast('Закрываю ' + (acc.name || 'аккаунт') + '…')
      setTimeout(loadAccounts, 400)
    } catch (e) {
      toast(e.message, true)
    }
    return
  }
  if (!acc.launchable) {
    toast('Нет сохранённого токена для этого аккаунта', true)
    return
  }
  if (launchingUuids.has(acc.uuid)) return
  launchingUuids.add(acc.uuid)
  try {
    await apiPost('/api/launch', { uuid: acc.uuid, client: client || acc.client || '' })
    toast('Запускаю ' + (acc.name || 'аккаунт') + '…')
    setTimeout(loadAccounts, 600)
  } catch (e) {
    toast(e.message, true)
  } finally {
    setTimeout(() => launchingUuids.delete(acc.uuid), 5000)
  }
}

function activeGroup() {
  if (!state.selectedGroup) return null
  return state.groups.find((g) => g.id === state.selectedGroup) || null
}

function accountGroupLabel(acc) {
  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  const g = primaryGroup(info)
  return g ? g : null
}

function availableGroups() {
  const map = new Map()
  state.accounts.forEach((a) => {
    const g = accountGroupLabel(a)
    if (g && !map.has(g.label)) map.set(g.label, g.color)
  })
  return Array.from(map.entries()).map(([label, color]) => ({ label, color }))
}

function applyFilters(list) {
  let out = list
  if (state.filters.groups.size || state.filters.noRole) {
    out = out.filter((a) => {
      const g = accountGroupLabel(a)
      if (g) return state.filters.groups.has(g.label)
      return state.filters.noRole
    })
  }
  if (state.filters.expired) {
    out = out.filter((a) => a.launchable && (state.debugExpired || a.expired))
  }
  return out
}

function applySort(list) {
  if (!state.sort.key) return list
  const dir = state.sort.dir === 'asc' ? 1 : -1
  const val = (a) => {
    if (state.sort.key === 'launched') return a.lastLaunched || 0
    if (state.sort.key === 'added') return a.firstSeen || 0
    if (state.sort.key === 'expiry') return a.expires || 0
    return 0
  }
  return list.slice().sort((a, b) => (val(a) - val(b)) * dir)
}

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

function activeFilterCount() {
  return state.filters.groups.size + (state.filters.noRole ? 1 : 0) + (state.filters.expired ? 1 : 0) + (state.sort.key ? 1 : 0)
}

function updateFilterBadge() {
  const badge = document.getElementById('filter-badge')
  const n = activeFilterCount()
  badge.hidden = n === 0
  badge.textContent = n
}

function renderFilterPanel() {
  const panel = document.getElementById('filter-panel')
  const groups = availableGroups()
  let html = '<div class="filter-section-title">Фильтр по группе</div>'
  if (!groups.length) {
    html += '<div class="filter-hint">Группы аккаунтов ещё не загружены</div>'
  } else {
    groups.forEach((g) => {
      const on = state.filters.groups.has(g.label)
      html +=
        '<button class="filter-check' + (on ? ' on' : '') + '" data-group="' + esc(g.label) + '">' +
        '<span class="filter-box" style="--chip:' + g.color + '">' + (on ? CHECK_SVG : '') + '</span>' +
        '<span class="filter-check-label" style="color:' + g.color + '">' + esc(g.label) + '</span></button>'
    })
  }
  html +=
    '<button class="filter-check' + (state.filters.noRole ? ' on' : '') + '" data-norole="1">' +
    '<span class="filter-box">' + (state.filters.noRole ? CHECK_SVG : '') + '</span>' +
    '<span class="filter-check-label">Без роли</span></button>'
  html +=
    '<button class="filter-check' + (state.filters.expired ? ' on' : '') + '" data-expired="1">' +
    '<span class="filter-box">' + (state.filters.expired ? CHECK_SVG : '') + '</span>' +
    '<span class="filter-check-label">Истёкшие токены</span></button>'

  html += '<div class="filter-sep"></div><div class="filter-section-title">Сортировка</div>'
  SORT_OPTIONS.forEach((o) => {
    const active = state.sort.key === o.key
    const arrowCls = active ? ' active' + (state.sort.dir === 'asc' ? ' up' : ' down') : ''
    html +=
      '<button class="filter-sort' + (active ? ' on' : '') + '" data-sort="' + o.key + '">' +
      '<span class="filter-sort-label">' + esc(o.label) + '</span>' +
      '<span class="filter-arrow' + arrowCls + '">' + ARROW_SVG + '</span></button>'
  })
  html += '<button class="filter-reset" id="filter-reset">' + RESET_SVG + '<span>Сбросить всё</span></button>'
  panel.innerHTML = html

  panel.querySelectorAll('.filter-check[data-group]').forEach((b) => {
    b.addEventListener('click', () => {
      const label = b.dataset.group
      if (state.filters.groups.has(label)) state.filters.groups.delete(label)
      else state.filters.groups.add(label)
      afterFilterChange()
    })
  })
  const noRoleBtn = panel.querySelector('.filter-check[data-norole]')
  if (noRoleBtn) {
    noRoleBtn.addEventListener('click', () => {
      state.filters.noRole = !state.filters.noRole
      afterFilterChange()
    })
  }
  const expBtn = panel.querySelector('.filter-check[data-expired]')
  if (expBtn) {
    expBtn.addEventListener('click', () => {
      state.filters.expired = !state.filters.expired
      afterFilterChange()
    })
  }
  panel.querySelectorAll('.filter-sort').forEach((b) => {
    b.addEventListener('click', () => {
      const key = b.dataset.sort
      if (state.sort.key !== key) {
        state.sort = { key, dir: 'desc' }
      } else if (state.sort.dir === 'desc') {
        state.sort.dir = 'asc'
      } else {
        state.sort = { key: null, dir: 'desc' }
      }
      afterFilterChange()
    })
  })
  document.getElementById('filter-reset').addEventListener('click', () => {
    state.filters.groups.clear()
    state.filters.noRole = false
    state.filters.expired = false
    state.sort = { key: null, dir: 'desc' }
    afterFilterChange()
  })
}

function afterFilterChange() {
  renderFilterPanel()
  updateFilterBadge()
  render()
}

function openFilterPanel() {
  renderFilterPanel()
  document.getElementById('filter-dd').classList.add('open')
  showMenu(document.getElementById('filter-panel'))
}
function closeFilterPanel() {
  document.getElementById('filter-dd').classList.remove('open')
  hideMenuAnimated(document.getElementById('filter-panel'))
}

function render() {
  const q = state.query.trim().toLowerCase()
  const group = activeGroup()
  let base = state.accounts
  if (group) {
    const mem = new Set(group.members || [])
    base = state.accounts.filter((a) => mem.has(a.uuid))
  }
  base = applyFilters(base)
  base = applySort(base)
  const list = q ? base.filter((a) => (a.name || a.uuid).toLowerCase().includes(q)) : base

  emptyBox.hidden = !!group || state.accounts.length !== 0
  const groupEmpty = document.getElementById('group-empty')
  const isEmptyGroup = !!group && base.length === 0
  if (groupEmpty) groupEmpty.hidden = !isEmptyGroup
  const topAdd = document.getElementById('group-add')
  if (topAdd) topAdd.hidden = isEmptyGroup

  const alive = new Set()
  list.forEach((acc, index) => {
    alive.add(acc.uuid)
    let card = state.cards.get(acc.uuid)
    if (!card) {
      card = buildCard(acc)
      state.cards.set(acc.uuid, card)
      card.classList.add('enter')
      const delay = state.firstRender ? Math.min(index * 24, 260) : 0
      if (delay) card.style.animationDelay = `${delay}ms`
      setTimeout(() => {
        card.classList.remove('enter')
        card.style.animationDelay = ''
      }, delay + 500)
    }
    fillCard(card, acc)
    if (grid.children[index] !== card) {
      grid.insertBefore(card, grid.children[index] || null)
    }
  })

  state.cards.forEach((card, uuid) => {
    if (!alive.has(uuid)) {
      card.remove()
      state.cards.delete(uuid)
    }
  })

  state.firstRender = false
}

function statTile(value, label, color) {
  return `<div class="stat-tile"><span class="stat-tile-value ${color || ''}">${value}</span><span class="stat-tile-label">${label}</span></div>`
}

function fmtDate(ts) {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  return d.toLocaleDateString('ru-RU') + ' ' + d.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })
}

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]))
}

function fmtDateTime(iso) {
  if (!iso) return '-'
  const d = new Date(iso)
  if (isNaN(d.getTime())) return '-'
  return (
    d.toLocaleDateString('ru-RU') +
    ' ' +
    d.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  )
}

function subLabel(key) {
  const m = /^level_(\d+)$/.exec(key || '')
  if (m) return 'Ур. ' + m[1]
  return key
}

function renderModalStats(acc) {
  const badge = accountBadge(acc)
  const statusColor = badge.cls === 'ready' ? 'green' : badge.cls === 'expired' ? 'yellow' : ''
  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  let tiles = ''
  if (info && info.online) {
    tiles += statTile(info.online === 'online' ? 'в сети' : 'не в сети', 'статус', info.online === 'online' ? 'green' : '')
  }
  tiles +=
    statTile(badge.text, 'статус токена', statusColor) +
    statTile(acc.running ? 'в игре' : '-', 'клиент', acc.running ? 'green' : '') +
    statTile(fmtDate(acc.expires), 'токен до', 'blue') +
    statTile(fmtDate(acc.lastLaunched), 'последний запуск')
  if (info) {
    if (info.subscription) tiles += statTile(subLabel(info.subscription), 'подписка', 'blue')
    if (info.likes || info.views) tiles += statTile(info.likes + ' / ' + info.views, 'лайки / просмотры')
    if (info.registeredAt) tiles += statTile(fmtDateTime(info.registeredAt), 'регистрация')
    if (info.lastSeen && info.online !== 'online') tiles += statTile(fmtDateTime(info.lastSeen), 'был онлайн')
  }
  document.getElementById('modal-stats').innerHTML = tiles
}

function renderModalGroups(acc) {
  const box = document.getElementById('modal-groups')
  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  if (!info) {
    box.innerHTML = ''
    return
  }
  const tags = []
  if (info.staff && info.staff !== 'PLAYER') {
    tags.push({ label: info.label || info.staff, color: info.color || '#8c8c8c' })
  }
  if (info.donate && info.donate !== 'NO') {
    tags.push({ label: info.donate, color: info.donateColor || '#e3a400' })
  }
  box.innerHTML = tags
    .map((t) => `<span class="grp-tag" style="--chip:${t.color}">${esc(t.label)}</span>`)
    .join('')
}

function openModal(acc) {
  state.selected = acc
  document.getElementById('modal-name').textContent = acc.name || shortId(acc.uuid) + '…'
  document.getElementById('modal-uuid').textContent = acc.uuid
  document.getElementById('modal-pin').classList.toggle('active', !!acc.pinned)
  document.getElementById('modal-label').value = acc.label || ''
  fillProfileSelect(acc)
  loadProfiles().then(() => {
    if (state.selected === acc) fillProfileSelect(acc)
  })
  setModalRam(acc)
  renderModalGroups(acc)
  renderModalStats(acc)
  if (acc.name) ensurePlayerInfo(acc.name)

  const note = document.getElementById('modal-note')
  const launchBtn = document.getElementById('modal-launch')
  if (acc.running) {
    launchBtn.textContent = 'Закрыть'
    launchBtn.className = 'action-btn stop'
    launchBtn.disabled = false
  } else if (acc.launching) {
    launchBtn.textContent = 'Запускается…'
    launchBtn.className = 'action-btn primary'
    launchBtn.disabled = true
  } else {
    launchBtn.textContent = 'Запуск'
    launchBtn.className = 'action-btn primary'
    launchBtn.disabled = !acc.launchable
  }
  if (!acc.launchable) {
    note.className = 'modal-note warn'
    note.textContent = 'Токен ещё не сохранён - зайди на этот аккаунт через лаунчер хотя бы раз.'
  } else if (acc.expired) {
    note.className = 'modal-note warn'
    note.textContent = 'Токен просрочен, вход может не сработать. Лаунчер попросит авторизацию заново.'
  } else {
    note.className = 'modal-note'
    note.textContent = ''
  }

  overlay.hidden = false
  mountSkin(acc.uuid)
}

let skinLibPromise = null
function loadSkinLib() {
  if (window.skinview3d) return Promise.resolve()
  if (!skinLibPromise) {
    skinLibPromise = new Promise((resolve, reject) => {
      const s = document.createElement('script')
      s.src = '/skinview3d.bundle.js'
      s.onload = resolve
      s.onerror = reject
      document.head.appendChild(s)
    })
  }
  return skinLibPromise
}

async function mountSkin(uuid) {
  disposeSkin()
  state.skinToken = uuid
  try {
    await loadSkinLib()
  } catch (e) {
    return
  }
  if (state.skinToken !== uuid) return
  const canvas = document.createElement('canvas')
  canvas.className = 'skin-canvas'
  skinStage.appendChild(canvas)

  const viewer = new skinview3d.SkinViewer({
    canvas,
    width: skinStage.clientWidth || 210,
    height: skinStage.clientHeight || 320,
  })
  state.viewer = viewer

  viewer.animation = new skinview3d.IdleAnimation()
  viewer.autoRotate = false
  viewer.controls.enableZoom = true
  viewer.controls.enablePan = false
  viewer.playerObject.rotation.y = 0.5
  viewer.zoom = 0.9

  const measure = () => ({
    width: skinStage.clientWidth || 210,
    height: skinStage.clientHeight || 320,
  })
  let lastW = 0
  let lastH = 0
  const ro = new ResizeObserver(() => {
    if (!state.viewer) return
    const { width, height } = measure()
    if (width <= 0 || height <= 0) return
    if (Math.abs(width - lastW) < 1 && Math.abs(height - lastH) < 1) return
    lastW = width
    lastH = height
    viewer.setSize(width, height)
  })
  ro.observe(skinStage)
  viewer._ro = ro

  viewer.loadSkin(`/skin/${uuid}`, { model: 'auto-detect' }).catch(() => {})
  viewer.loadCape(`/cape/${uuid}`).catch(() => {})
}

function disposeSkin() {
  state.skinToken = null
  if (state.viewer) {
    if (state.viewer._ro) state.viewer._ro.disconnect()
    state.viewer.dispose()
    state.viewer = null
  }
  skinStage.innerHTML = ''
}

function closeModal() {
  overlay.hidden = true
  disposeSkin()
  state.selected = null
}

async function loadAccounts() {
  try {
    const accounts = await apiGet('/api/accounts')
    state.accounts = accounts
    state.byUuid = new Map(accounts.map((a) => [a.uuid, a]))
    render()
  } catch (e) {
    /* keep last list */
  }
}

async function loadGroups() {
  try {
    const data = await apiGet('/api/groups')
    state.groups = data.groups || []
  } catch (e) {
    state.groups = []
  }
  if (state.selectedGroup && !state.groups.find((g) => g.id === state.selectedGroup)) {
    state.selectedGroup = null
  }
  renderGroupBar()
  if (document.getElementById('group-dd').classList.contains('open')) renderGroupMenu()
  render()
}

function renderGroupBar() {
  const group = activeGroup()
  document.getElementById('group-value').textContent = group ? group.name : 'Все аккаунты'
  document.getElementById('group-actions').hidden = !group
  if (group && state.groupProfileDD) {
    const names = state.profiles.slice()
    if (group.profile && !names.includes(group.profile)) names.unshift(group.profile)
    const options = [{ value: '', label: '- без профиля -' }].concat(names.map((p) => ({ value: p, label: p })))
    state.groupProfileDD.set(options, group.profile || '')
  }
}

function renderGroupMenu() {
  const menu = document.getElementById('group-menu')
  let html =
    '<button class="group-item' +
    (!state.selectedGroup ? ' active' : '') +
    '" data-id=""><span class="group-item-name">Все аккаунты</span></button>'
  state.groups.forEach((g) => {
    html +=
      '<button class="group-item' +
      (state.selectedGroup === g.id ? ' active' : '') +
      '" data-id="' +
      esc(g.id) +
      '">' +
      (g.pinned ? '<span class="group-pin-mark">' + STAR_SVG + '</span>' : '') +
      '<span class="group-item-name">' +
      esc(g.name) +
      '</span><span class="group-item-count">' +
      (g.members ? g.members.length : 0) +
      '</span></button>'
  })
  html +=
    '<div class="group-menu-sep"></div>' +
    '<button class="group-item group-item-action" id="group-create-item">＋ Создать группу</button>' +
    '<button class="group-item group-item-action" id="group-manage-item">⚙ Управление группами</button>'
  menu.innerHTML = html
  menu.querySelectorAll('.group-item[data-id]').forEach((item) => {
    item.addEventListener('click', () => {
      selectGroup(item.dataset.id || null)
      closeGroupMenu()
    })
  })
  document.getElementById('group-create-item').addEventListener('click', () => {
    closeGroupMenu()
    createGroupFlow()
  })
  document.getElementById('group-manage-item').addEventListener('click', () => {
    closeGroupMenu()
    openGroupsManage()
  })
}

function openGroupMenu() {
  renderGroupMenu()
  const dd = document.getElementById('group-dd')
  const menu = document.getElementById('group-menu')
  dd.classList.add('open')
  showMenu(menu)
  menu.classList.remove('flip-up')
  menu.style.maxHeight = ''
  const trigger = document.getElementById('group-trigger').getBoundingClientRect()
  const gap = 6
  const margin = 12
  const below = window.innerHeight - trigger.bottom - gap - margin
  const above = trigger.top - gap - margin
  if (below < 200 && above > below) {
    menu.classList.add('flip-up')
    menu.style.maxHeight = Math.min(340, Math.max(120, above)) + 'px'
  } else {
    menu.style.maxHeight = Math.min(340, Math.max(120, below)) + 'px'
  }
}
function closeGroupMenu() {
  document.getElementById('group-dd').classList.remove('open')
  hideMenuAnimated(document.getElementById('group-menu'))
}

function selectGroup(id) {
  state.selectedGroup = id
  renderGroupBar()
  render()
  replayCardEnter()
}

function replayCardEnter() {
  const cards = Array.from(grid.children).filter((c) => c.classList.contains('acc-card'))
  cards.forEach((card, i) => {
    card.classList.remove('enter')
    void card.offsetWidth
    const delay = Math.min(i * 26, 300)
    card.style.animationDelay = delay + 'ms'
    card.classList.add('enter')
    setTimeout(() => {
      card.classList.remove('enter')
      card.style.animationDelay = ''
    }, delay + 400)
  })
}

async function createGroupFlow() {
  const name = await askName('Название новой группы', '')
  if (!name || !name.trim()) return
  try {
    const r = await apiPost('/api/groups/create', { name: name.trim() })
    await loadGroups()
    if (r.group) selectGroup(r.group.id)
    toast('Группа создана')
  } catch (e) {
    toast(e.message, true)
  }
}

async function launchActiveGroup() {
  const group = activeGroup()
  if (!group) return
  try {
    await apiPost('/api/groups/launch', { id: group.id })
    toast('Запускаю аккаунты группы поочерёдно…')
  } catch (e) {
    toast(e.message, true)
  }
}

function addTargetGroup() {
  const id = state.addTargetGroupId || state.selectedGroup
  return state.groups.find((g) => g.id === id) || null
}

function openAddMembers(groupId) {
  state.addTargetGroupId = groupId || state.selectedGroup
  const group = addTargetGroup()
  if (!group) return
  document.getElementById('addmembers-title').textContent = 'Добавить в «' + group.name + '»'
  document.getElementById('addmembers-search').value = ''
  document.getElementById('addmembers-overlay').hidden = false
  renderAddMembers()
  setTimeout(() => document.getElementById('addmembers-search').focus(), 40)
}
function closeAddMembers() {
  document.getElementById('addmembers-overlay').hidden = true
  state.addTargetGroupId = null
}

function renderAddMembers() {
  const group = addTargetGroup()
  if (!group) return
  const q = document.getElementById('addmembers-search').value.trim().toLowerCase()
  const mem = new Set(group.members || [])
  const list = state.accounts
    .filter((a) => !mem.has(a.uuid))
    .filter((a) => !q || (a.name || a.uuid).toLowerCase().includes(q))
  const box = document.getElementById('addmembers-list')
  box.innerHTML = ''
  if (!list.length) {
    box.innerHTML = '<div class="addmembers-empty">Все аккаунты уже в группе</div>'
    return
  }
  list.forEach((a, i) => {
    const card = buildPickerCard(a, async () => {
      try {
        await apiPost('/api/groups/members', { id: group.id, add: a.uuid })
        await loadGroups()
        renderAddMembers()
        if (!document.getElementById('groups-overlay').hidden) renderGroupsManage()
      } catch (e) {
        toast(e.message, true)
      }
    })
    card.style.animationDelay = Math.min(i * 26, 300) + 'ms'
    box.appendChild(card)
  })
}

function buildPickerCard(acc, onAdd) {
  const card = document.createElement('div')
  card.className = 'acc-card picker-card'

  const avatarWrap = document.createElement('span')
  avatarWrap.className = 'avatar-wrap'
  const avatar = document.createElement('span')
  avatar.className = 'avatar'
  avatar.style.backgroundImage = skinBg(acc.uuid)
  const hat = document.createElement('span')
  hat.className = 'avatar-hat'
  hat.style.backgroundImage = skinBg(acc.uuid)
  avatar.appendChild(hat)
  const dot = document.createElement('span')
  dot.className = 'online-dot'
  avatarWrap.appendChild(avatar)
  avatarWrap.appendChild(dot)

  const body = document.createElement('div')
  body.className = 'acc-body'
  const title = document.createElement('div')
  title.className = 'acc-title'
  const chip = document.createElement('span')
  chip.className = 'grp-chip'
  chip.hidden = true
  const name = document.createElement('div')
  name.className = acc.name ? 'acc-name' : 'acc-name unknown'
  name.textContent = acc.name || shortId(acc.uuid) + '…'
  title.appendChild(chip)
  title.appendChild(name)
  const meta = document.createElement('div')
  meta.className = 'acc-meta'
  const badge = accountBadge(acc)
  const badgeEl = document.createElement('span')
  badgeEl.className = 'badge ' + badge.cls
  badgeEl.textContent = badge.text
  meta.appendChild(badgeEl)
  body.appendChild(title)
  body.appendChild(meta)

  const addBtn = document.createElement('button')
  addBtn.className = 'play-btn picker-add'
  addBtn.innerHTML = PLUS_SVG
  addBtn.title = 'Добавить в группу'

  card.appendChild(avatarWrap)
  card.appendChild(body)
  card.appendChild(addBtn)

  const add = (e) => {
    e.stopPropagation()
    onAdd()
  }
  card.addEventListener('click', add)

  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  const grp = primaryGroup(info)
  if (grp) {
    chip.hidden = false
    chip.textContent = grp.label
    chip.style.setProperty('--chip', grp.color)
    card.style.setProperty('--grp', grp.color)
  }
  const online = info ? info.online : ''
  dot.className = 'online-dot' + (online === 'online' ? ' on' : online === 'offline' ? ' off' : '')
  if (acc.name) ensurePlayerInfo(acc.name)
  return card
}

function openGroupsManage() {
  document.getElementById('groups-overlay').hidden = false
  renderGroupsManage()
}
function closeGroupsManage() {
  document.getElementById('groups-overlay').hidden = true
}

const XMARK_SVG =
  '<svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M5 5l14 14M19 5L5 19"/></svg>'

function renderGroupsManage() {
  const box = document.getElementById('groups-manage-list')
  box.innerHTML = ''
  if (!state.groups.length) {
    box.innerHTML = '<div class="addmembers-empty">Групп пока нет. Создай первую.</div>'
    return
  }
  state.groups.forEach((g, gi) => {
    const expanded = state.gmExpanded === g.id
    const wrap = document.createElement('div')
    wrap.className = 'gm-group'
    wrap.draggable = true
    wrap.dataset.id = g.id
    wrap.style.animationDelay = Math.min(gi * 30, 300) + 'ms'

    const row = document.createElement('div')
    row.className = 'gm-row'
    row.innerHTML =
      '<span class="gm-drag" title="Перетащить">⋮⋮</span>' +
      '<button class="gm-pin' + (g.pinned ? ' on' : '') + '" data-act="pin" title="Закрепить">' + STAR_SVG + '</button>' +
      '<span class="gm-name">' + esc(g.name) + '</span>' +
      '<button class="gm-members-toggle' + (expanded ? ' open' : '') + '" data-act="toggle">' +
      (g.members ? g.members.length : 0) + ' акк.' + CARET_SVG + '</button>' +
      '<button class="gm-btn" data-act="rename">Переименовать</button>' +
      '<button class="gm-btn danger" data-act="delete">Удалить</button>'
    wrap.appendChild(row)

    if (expanded) {
      const panel = document.createElement('div')
      panel.className = 'gm-members'
      const members = (g.members || []).map((uuid) => state.byUuid.get(uuid)).filter(Boolean)
      if (!members.length) {
        const empty = document.createElement('div')
        empty.className = 'gm-members-empty'
        empty.textContent = 'В группе нет аккаунтов'
        panel.appendChild(empty)
      }
      members.forEach((acc, mi) => {
        const mrow = document.createElement('div')
        mrow.className = 'gm-member'
        mrow.style.animationDelay = Math.min(mi * 24, 240) + 'ms'
        mrow.innerHTML =
          '<span class="gm-member-ava head-ava" style="--skin:' + skinBg(acc.uuid) + '"></span>' +
          '<span class="gm-member-name">' + esc(acc.name || shortId(acc.uuid)) + '</span>' +
          '<button class="gm-member-remove" title="Убрать из группы">' + XMARK_SVG + '</button>'
        mrow.querySelector('.gm-member-remove').addEventListener('click', async () => {
          try {
            await apiPost('/api/groups/members', { id: g.id, remove: acc.uuid })
            await loadGroups()
            renderGroupsManage()
          } catch (e) {
            toast(e.message, true)
          }
        })
        panel.appendChild(mrow)
      })
      const addBtn = document.createElement('button')
      addBtn.className = 'gm-add-members'
      addBtn.innerHTML = PLUS_SVG + '<span>Добавить аккаунты</span>'
      addBtn.addEventListener('click', () => openAddMembers(g.id))
      panel.appendChild(addBtn)
      wrap.appendChild(panel)
    }

    row.querySelector('[data-act="toggle"]').addEventListener('click', () => {
      state.gmExpanded = expanded ? null : g.id
      renderGroupsManage()
    })
    row.querySelector('[data-act="pin"]').addEventListener('click', async () => {
      await apiPost('/api/groups/pin', { id: g.id, pinned: !g.pinned })
      await loadGroups()
      renderGroupsManage()
    })
    row.querySelector('[data-act="rename"]').addEventListener('click', async () => {
      const name = await askName('Новое название группы', g.name)
      if (!name || !name.trim()) return
      await apiPost('/api/groups/rename', { id: g.id, name: name.trim() })
      await loadGroups()
      renderGroupsManage()
    })
    row.querySelector('[data-act="delete"]').addEventListener('click', async () => {
      const ok = await confirmDialog('Удалить группу?', 'Вы уверены, что хотите удалить группу «' + esc(g.name) + '»?')
      if (!ok) return
      await apiPost('/api/groups/delete', { id: g.id })
      if (state.selectedGroup === g.id) state.selectedGroup = null
      if (state.gmExpanded === g.id) state.gmExpanded = null
      await loadGroups()
      renderGroupsManage()
    })
    wrap.addEventListener('dragstart', () => wrap.classList.add('dragging'))
    wrap.addEventListener('dragend', async () => {
      wrap.classList.remove('dragging')
      const ids = Array.from(box.querySelectorAll('.gm-group')).map((r) => r.dataset.id)
      await apiPost('/api/groups/reorder', { ids })
      await loadGroups()
      renderGroupsManage()
    })
    box.appendChild(wrap)
  })
}

function groupDragOver(e) {
  e.preventDefault()
  const box = document.getElementById('groups-manage-list')
  const dragging = box.querySelector('.gm-group.dragging')
  if (!dragging) return
  const rows = Array.from(box.querySelectorAll('.gm-group:not(.dragging)'))
  const after = rows.find((r) => {
    const rect = r.getBoundingClientRect()
    return e.clientY < rect.top + rect.height / 2
  })
  if (after) box.insertBefore(dragging, after)
  else box.appendChild(dragging)
}

const logsState = { open: false, uuid: null, timer: null }

function openLogs() {
  logsState.open = true
  document.getElementById('logs-overlay').hidden = false
  refreshLogsSide()
  if (!logsState.uuid) {
    const view = document.getElementById('logs-view')
    view.classList.add('is-empty')
    view.textContent = 'Выбери аккаунт слева, чтобы увидеть его лог'
  }
  if (logsState.timer) clearInterval(logsState.timer)
  logsState.timer = setInterval(() => {
    if (!logsState.open) return
    refreshLogsSide()
    if (logsState.uuid) refreshLogsView()
  }, 1500)
}

function closeLogs() {
  logsState.open = false
  document.getElementById('logs-overlay').hidden = true
  if (logsState.timer) {
    clearInterval(logsState.timer)
    logsState.timer = null
  }
}

async function refreshLogsSide() {
  let logs = []
  try {
    const data = await apiGet('/api/logs')
    logs = data.logs || []
  } catch (e) {
    return
  }
  logs.sort((a, b) => Number(b.active) - Number(a.active) || (a.name || '').localeCompare(b.name || ''))
  const side = document.getElementById('logs-side')
  if (!logs.length) {
    side.innerHTML = '<div class="logs-empty">Пока никого не запускали через ченджер</div>'
    return
  }
  side.innerHTML = logs
    .map((l) => {
      const acc = state.byUuid.get(l.uuid)
      const g = acc ? accountGroupLabel(acc) : null
      const chip = g
        ? '<span class="logs-chip" style="--chip:' + g.color + ';color:' + g.color + '">' + esc(g.label) + '</span>'
        : ''
      return (
        '<button class="logs-item' +
        (logsState.uuid === l.uuid ? ' active' : '') +
        '" data-uuid="' +
        esc(l.uuid) +
        '"><span class="logs-dot' +
        (l.active ? ' on' : '') +
        '"></span><span class="logs-item-ava head-ava" style="--skin:' +
        skinBg(l.uuid) +
        '"></span>' +
        chip +
        '<span class="logs-item-name">' +
        esc(l.name || shortId(l.uuid)) +
        '</span><span class="logs-item-count">' +
        fmtCount(l.lines) +
        '</span></button>'
      )
    })
    .join('')
  side.querySelectorAll('.logs-item').forEach((item) => {
    item.addEventListener('click', () => selectLog(item.dataset.uuid))
  })
}

function selectLog(uuid) {
  logsState.uuid = uuid
  const acc = state.byUuid.get(uuid)
  document.getElementById('logs-current').textContent = acc ? acc.name || shortId(uuid) : shortId(uuid)
  document.getElementById('logs-clear').hidden = false
  document.querySelectorAll('#logs-side .logs-item').forEach((i) => i.classList.toggle('active', i.dataset.uuid === uuid))
  refreshLogsView(true)
}

async function refreshLogsView(force) {
  if (!logsState.uuid) return
  let lines = []
  try {
    const data = await apiGet('/api/logs/get?uuid=' + encodeURIComponent(logsState.uuid))
    lines = data.lines || []
  } catch (e) {
    return
  }
  const view = document.getElementById('logs-view')
  if (!lines.length) {
    view.classList.add('is-empty')
    view.textContent = 'Лог пуст'
    return
  }
  view.classList.remove('is-empty')
  const newText = lines.map((l) => l.text).join('\n')
  if (view.textContent === newText) return
  if (!force) {
    const sel = window.getSelection()
    if (sel && !sel.isCollapsed && sel.rangeCount && view.contains(sel.anchorNode)) return
  }
  const atBottom = view.scrollTop + view.clientHeight >= view.scrollHeight - 40
  view.textContent = newText
  if (force || atBottom) view.scrollTop = view.scrollHeight
}

async function copyText(text) {
  if (typeof window.acCopy === 'function') {
    try {
      const ok = await window.acCopy(text)
      if (ok !== false) return true
    } catch (e) {
      /* fall through */
    }
  }
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch (e) {
    try {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      const ok = document.execCommand('copy')
      document.body.removeChild(ta)
      return ok
    } catch (e2) {
      return false
    }
  }
}

document.getElementById('uuid-row').addEventListener('click', async () => {
  const row = document.getElementById('uuid-row')
  const uuid = document.getElementById('modal-uuid').textContent.trim()
  if (!uuid) return
  const ok = await copyText(uuid)
  if (ok) {
    row.classList.add('copied')
    toast('UUID скопирован')
    clearTimeout(row._t)
    row._t = setTimeout(() => row.classList.remove('copied'), 1200)
  } else {
    toast('Не удалось скопировать', true)
  }
})

document.getElementById('modal-close').addEventListener('click', closeModal)
overlay.addEventListener('click', (e) => {
  if (e.target === overlay) closeModal()
})
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return
  if (!document.getElementById('prompt-overlay').hidden) {
    closePrompt(null)
    return
  }
  if (!document.getElementById('profiles-overlay').hidden) {
    closeProfiles()
    return
  }
  if (!overlay.hidden) closeModal()
})

searchInput.addEventListener('input', () => {
  state.query = searchInput.value
  render()
})

document.getElementById('modal-launch').addEventListener('click', () => {
  if (!state.selected) return
  togglePlay(state.selected)
  closeModal()
})

document.getElementById('modal-profile').addEventListener('click', () => {
  if (state.selected && state.selected.name) openProfile(state.selected.name)
})

document.getElementById('modal-pin').addEventListener('click', async () => {
  if (!state.selected) return
  const next = !state.selected.pinned
  document.getElementById('modal-pin').classList.toggle('active', next)
  try {
    await apiPost('/api/pin', { uuid: state.selected.uuid, pinned: next })
    state.selected.pinned = next
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
})

const labelInput = document.getElementById('modal-label')
async function saveLabel() {
  if (!state.selected) return
  const value = labelInput.value.trim()
  if (value === (state.selected.label || '')) return
  try {
    await apiPost('/api/label', { uuid: state.selected.uuid, label: value })
    state.selected.label = value
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
}
labelInput.addEventListener('blur', saveLabel)
labelInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') labelInput.blur()
})

document.getElementById('modal-forget').addEventListener('click', async () => {
  if (!state.selected) return
  const acc = state.selected
  const ok = await confirmDialog('Удалить аккаунт?', 'Аккаунт «' + esc(acc.name || shortId(acc.uuid)) + '» будет убран из списка.')
  if (!ok) return
  try {
    await apiPost('/api/forget', { uuid: acc.uuid })
    toast('Аккаунт убран из списка')
    closeModal()
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
})

document.getElementById('guest-btn').addEventListener('click', async () => {
  try {
    await apiPost('/api/launch-guest')
    toast('Запускаю лаунчер без аккаунта…')
  } catch (e) {
    toast(e.message, true)
  }
})

document.getElementById('launcher-btn').addEventListener('click', async () => {
  try {
    await apiPost('/api/launcher')
    toast('Открываю лаунчер…')
  } catch (e) {
    toast(e.message, true)
  }
})

document.getElementById('refresh-tokens-btn').addEventListener('click', async () => {
  const btn = document.getElementById('refresh-tokens-btn')
  if (btn.disabled) return
  btn.disabled = true
  try {
    const r = await apiPost('/api/accounts/refresh-start')
    if (!r.ok) {
      toast(r.error || 'нечего обновлять', true)
      return
    }
    toast('Обновляю истекающие токены - продолжит в фоне')
    importCancelling = false
    ensureImportPolling()
    await pollImport()
  } catch (e) {
    toast(e.message, true)
  } finally {
    btn.disabled = false
  }
})

state.profileDD = initDropdown('modal-profile-select', async (value) => {
  if (!state.selected) return
  try {
    await apiPost('/api/account/profile', { uuid: state.selected.uuid, profile: value })
    state.selected.profile = value
    loadAccounts()
    toast(value ? 'Профиль назначен: ' + value : 'Профиль убран')
  } catch (err) {
    toast(err.message, true)
  }
})

function openUrl(url) {
  if (typeof window.acOpenUrl === 'function') {
    window.acOpenUrl(url)
  } else {
    window.open(url, '_blank')
  }
}

document.getElementById('footer-link').addEventListener('click', (e) => {
  e.preventDefault()
  openUrl(e.currentTarget.href)
})
document.getElementById('logo-link').addEventListener('click', () => {
  openUrl('https://cristalix.gg')
})
document.getElementById('brand-link').addEventListener('click', () => {
  openUrl('https://github.com/xpepelok/cristalix-account-changer')
})
document.getElementById('tg-link').addEventListener('click', () => {
  openUrl('https://t.me/xpplkn')
})

async function checkUpdate() {
  try {
    const info = await apiGet('/api/update')
    state.update = info
    if (info && info.available) {
      setBellNotif(true)
      toast('Доступно обновление ' + info.latest)
    }
  } catch (e) {
    /* ignore */
  }
}

function setBellNotif(active) {
  document.getElementById('bell-dot').hidden = !active
  document.getElementById('bell-btn').classList.toggle('has-notif', active)
}

function renderNotifPanel() {
  const panel = document.getElementById('notif-panel')
  const u = state.update
  if (!u || !u.available) {
    panel.innerHTML = '<div class="notif-empty">Новых уведомлений нет</div>'
    return
  }
  const notes = u.notes && u.notes.trim() ? u.notes.trim() : 'Новая версия готова к установке.'
  const desc = 'Текущая версия: ' + esc(u.current) + '\n\n' + esc(notes)
  panel.innerHTML =
    '<div class="notif-card">' +
    '<div class="notif-title">Доступно обновление ' +
    esc(u.latest) +
    '</div>' +
    '<div class="notif-desc">' +
    desc +
    '</div>' +
    '<button class="notif-download" id="update-download" title="Скачать и установить">' +
    DOWNLOAD_SVG +
    '</button>' +
    '</div>'
  document.getElementById('update-download').addEventListener('click', doUpdate)
}

function toggleBell() {
  const wrap = document.querySelector('.bell-wrap')
  const panel = document.getElementById('notif-panel')
  const open = wrap.classList.toggle('open')
  if (open) {
    renderNotifPanel()
    showMenu(panel)
  } else {
    hideMenuAnimated(panel)
  }
}

async function doUpdate() {
  const btn = document.getElementById('update-download')
  if (btn) btn.disabled = true
  const card = document.querySelector('.notif-card')
  let bar = document.getElementById('notif-progress')
  if (!bar && card) {
    card.insertAdjacentHTML('beforeend', '<div class="notif-progress" id="notif-progress"><div class="notif-progress-fill" id="notif-progress-fill"></div><span class="notif-progress-text" id="notif-progress-text">Подготовка…</span></div>')
    bar = document.getElementById('notif-progress')
  }
  try {
    await apiPost('/api/update/apply')
  } catch (e) {
    toast(e.message, true)
    if (btn) btn.disabled = false
    if (bar) bar.remove()
    return
  }
  const fill = document.getElementById('notif-progress-fill')
  const text = document.getElementById('notif-progress-text')
  const poll = setInterval(async () => {
    let p
    try {
      p = await apiGet('/api/update/progress')
    } catch (e) {
      return
    }
    if (p.error) {
      clearInterval(poll)
      toast(p.error, true)
      if (fill) fill.style.width = '0%'
      if (text) text.textContent = 'Ошибка загрузки'
      if (btn) btn.disabled = false
      return
    }
    if (p.done) {
      clearInterval(poll)
      if (fill) fill.style.width = '100%'
      if (text) text.textContent = 'Установлено, перезапуск…'
      toast('Обновление установлено, перезапуск…')
      return
    }
    if (p.active) {
      const pct = p.total > 0 ? p.percent : 0
      if (fill) fill.style.width = pct + '%'
      if (text) {
        text.textContent =
          p.total > 0
            ? 'Скачивание ' + pct + '% (' + fmtBytes(p.downloaded) + ' / ' + fmtBytes(p.total) + ')'
            : 'Скачивание ' + fmtBytes(p.downloaded) + '…'
      }
    }
  }, 300)
}

function fmtBytes(n) {
  if (!n || n < 1024) return (n || 0) + ' Б'
  if (n < 1024 * 1024) return (n / 1024).toFixed(0) + ' КБ'
  return (n / (1024 * 1024)).toFixed(1) + ' МБ'
}

document.getElementById('bell-btn').addEventListener('click', (e) => {
  e.stopPropagation()
  toggleBell()
})
document.addEventListener('click', (e) => {
  const wrap = document.querySelector('.bell-wrap')
  if (wrap.classList.contains('open') && !wrap.contains(e.target)) {
    wrap.classList.remove('open')
    hideMenuAnimated(document.getElementById('notif-panel'))
  }
})

document.getElementById('win-min').addEventListener('click', () => {
  if (typeof window.acMinimize === 'function') window.acMinimize()
})
document.getElementById('win-max').addEventListener('click', () => {
  if (typeof window.acMaximize === 'function') window.acMaximize()
  const btn = document.getElementById('win-max')
  const on = btn.classList.toggle('maximized')
  btn.querySelector('.ic-max').hidden = on
  btn.querySelector('.ic-restore').hidden = !on
  btn.title = on ? 'Свернуть в окно' : 'Развернуть'
})
document.getElementById('win-close').addEventListener('click', () => {
  if (typeof window.acHide === 'function') window.acHide()
})
document.querySelector('.topbar .inner').addEventListener('mousedown', (e) => {
  if (e.button !== 0) return
  if (e.target.closest('button, a, input, select, .ac-dd, .logo, .brand-tag, .active-badge')) return
  if (typeof window.acDrag === 'function') window.acDrag()
})

document.getElementById('profiles-btn').addEventListener('click', openProfiles)
document.getElementById('profiles-close').addEventListener('click', closeProfiles)
document.getElementById('profiles-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'profiles-overlay') closeProfiles()
})
document.getElementById('profile-save-current').addEventListener('click', async () => {
  const name = await askName('Название нового профиля', '')
  if (!name || !name.trim()) return
  try {
    const r = await apiPost('/api/profiles/save', { name: name.trim() })
    toast('Сохранено файлов: ' + r.copied)
    await renderProfilesList()
    editProfile(r.name)
  } catch (e) {
    toast(e.message, true)
  }
})

const IMPORT_FILES = ['binds.json', 'options.txt', 'optionsof.txt', 'voicechat.json']

function openImport() {
  state.importFiles = {}
  document.getElementById('import-name').value = ''
  document.getElementById('import-file-input').value = ''
  renderImportFiles()
  document.getElementById('import-overlay').hidden = false
  setTimeout(() => document.getElementById('import-name').focus(), 30)
}
function closeImport() {
  document.getElementById('import-overlay').hidden = true
}
function renderImportFiles() {
  const files = state.importFiles || {}
  const names = Object.keys(files)
  document.getElementById('import-files').innerHTML = names
    .map(
      (n) =>
        `<div class="import-file-chip"><span class="chip-ok">✓</span>${esc(n)}<span class="chip-rm" data-file="${n}" title="Убрать">✕</span></div>`,
    )
    .join('')
  document.querySelectorAll('#import-files .chip-rm').forEach((el) => {
    el.addEventListener('click', () => {
      delete state.importFiles[el.dataset.file]
      renderImportFiles()
    })
  })
  const name = document.getElementById('import-name').value.trim()
  document.getElementById('import-apply').disabled = !(name && names.length)
}
function ingestImportFiles(fileList) {
  const files = Array.from(fileList || [])
  if (!files.length) return
  state.importFiles = state.importFiles || {}
  const skipped = []
  let pending = files.length
  const done = () => {
    renderImportFiles()
    if (skipped.length) toast('Пропущено (не файлы настроек): ' + skipped.join(', '), true)
  }
  files.forEach((file) => {
    const key = (file.name || '').toLowerCase()
    if (!IMPORT_FILES.includes(key)) {
      skipped.push(file.name)
      if (--pending === 0) done()
      return
    }
    const reader = new FileReader()
    reader.onload = () => {
      state.importFiles[key] = String(reader.result || '')
      if (--pending === 0) done()
    }
    reader.onerror = () => {
      if (--pending === 0) done()
    }
    reader.readAsText(file)
  })
}
document.getElementById('profile-import').addEventListener('click', openImport)
document.getElementById('import-close').addEventListener('click', closeImport)
document.getElementById('import-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'import-overlay') closeImport()
})
document.getElementById('import-name').addEventListener('input', renderImportFiles)
const importDrop = document.getElementById('import-drop')
const importInput = document.getElementById('import-file-input')
importDrop.addEventListener('click', () => importInput.click())
importInput.addEventListener('change', (e) => {
  ingestImportFiles(e.target.files)
  e.target.value = ''
})
importDrop.addEventListener('dragover', (e) => {
  e.preventDefault()
  importDrop.classList.add('drag')
})
importDrop.addEventListener('dragleave', () => importDrop.classList.remove('drag'))
importDrop.addEventListener('drop', (e) => {
  e.preventDefault()
  importDrop.classList.remove('drag')
  ingestImportFiles(e.dataTransfer && e.dataTransfer.files)
})
document.getElementById('import-apply').addEventListener('click', async () => {
  const name = document.getElementById('import-name').value.trim()
  const files = state.importFiles || {}
  if (!name || !Object.keys(files).length) return
  try {
    await apiPost('/api/profiles/update', { name, files })
    toast('Профиль импортирован: ' + name)
    closeImport()
    await renderProfilesList()
    editProfile(name)
  } catch (e) {
    toast(e.message, true)
  }
})
let credList = []
let credEntryIndex = -1
let importSnap = null
let importTimer = null

function importActive(snap) {
  return snap && snap.items && snap.items.length && (snap.running || snap.done < snap.total || snap.ok || snap.fail)
}
function ensureImportPolling() {
  if (importTimer) return
  importTimer = setInterval(pollImport, 1000)
}
async function pollImport() {
  let snap
  try {
    snap = await apiGet('/api/accounts/import-progress')
  } catch (e) {
    return
  }
  importSnap = snap
  const fab = document.getElementById('cred-btn')
  fab.classList.toggle('loading', !!snap.running)
  const badge = document.getElementById('cred-badge')
  if (snap.running && snap.total) {
    badge.hidden = false
    badge.textContent = snap.done + '/' + snap.total
  } else {
    badge.hidden = true
  }
  const overlay = document.getElementById('cred-overlay')
  if (!overlay.hidden && importActive(snap)) renderImportProgress(snap)
  if (!snap.running) {
    clearInterval(importTimer)
    importTimer = null
    loadAccounts()
    if (importCancelling) {
      importCancelling = false
      importSnap = null
      document.getElementById('cred-overlay').hidden = true
    }
  } else {
    loadAccounts()
  }
}
function renderImportProgress(snap) {
  document.getElementById('cred-add').disabled = true
  document.getElementById('cred-drop').style.pointerEvents = 'none'
  document.getElementById('cred-apply').hidden = true
  const cancelBtn = document.getElementById('cred-cancel')
  cancelBtn.hidden = !snap.running
  cancelBtn.disabled = importCancelling
  cancelBtn.textContent = importCancelling ? 'Отменяю…' : 'Отменить импорт'
  document.getElementById('cred-newlist').hidden = !!snap.running
  const box = document.getElementById('cred-list')
  box.innerHTML = (snap.items || [])
    .map((it) => {
      const cls =
        it.status === 'ok'
          ? 'ok'
          : it.status === 'err' || it.status === 'canceled'
          ? 'err'
          : it.status === 'skip'
          ? 'skip'
          : 'working'
      const statusText = it.status === 'ok' ? '✓' : it.text || it.status
      return (
        '<div class="cred-item">' +
        '<div class="cred-item-main"><span class="cred-item-login">' + esc(it.login) + '</span></div>' +
        '<span class="cred-item-status ' + cls + '">' + esc(statusText) + '</span>' +
        '</div>'
      )
    })
    .join('')
  document.getElementById('cred-count').textContent =
    'Импорт: ' + snap.ok + ' сохранено' +
    (snap.skip ? ' · ' + snap.skip + ' уже было' : '') +
    (snap.fail ? ' · ' + snap.fail + ' с ошибкой' : '')
  const prog = document.getElementById('cred-progress')
  prog.hidden = false
  document.getElementById('cred-progress-text').textContent = snap.running
    ? snap.done + ' / ' + snap.total + ' · вход в аккаунты…'
    : 'Готово: ' + (snap.ok + snap.skip) + ' из ' + snap.total + (snap.fail ? ' (' + snap.fail + ' с ошибкой)' : '')
  document.getElementById('cred-progress-fill').style.width =
    Math.round((snap.done / Math.max(1, snap.total)) * 100) + '%'
}

function renderCredList() {
  const box = document.getElementById('cred-list')
  document.getElementById('cred-count').textContent = 'Подготовлено: ' + credList.length
  const applyBtn = document.getElementById('cred-apply')
  if (applyBtn) applyBtn.textContent = credList.length ? 'Импортировать (' + credList.length + ')' : 'Импортировать'
  if (!credList.length) {
    box.innerHTML = '<div class="cred-list-empty">Список пуст. Добавь аккаунт или перетащи .txt</div>'
    return
  }
  box.innerHTML = credList
    .map((c, i) => {
      const st = c.status
        ? '<span class="cred-item-status ' + (st_cls(c.status)) + '">' + esc(c.statusText || '') + '</span>'
        : ''
      return (
        '<div class="cred-item" data-i="' + i + '">' +
        '<div class="cred-item-main" data-edit="' + i + '">' +
        '<span class="cred-item-login">' + esc(c.login) + '</span>' +
        '<span class="cred-item-sub">' + '•'.repeat(Math.min(8, (c.password || '').length)) + '</span>' +
        '</div>' + st +
        '<button class="cred-item-rm" data-rm="' + i + '" title="Убрать" tabindex="-1">✕</button>' +
        '</div>'
      )
    })
    .join('')
  box.querySelectorAll('.cred-item-main').forEach((el) => {
    el.addEventListener('click', () => openCredEntry(+el.dataset.edit))
  })
  box.querySelectorAll('.cred-item-rm').forEach((el) => {
    el.addEventListener('click', () => {
      credList.splice(+el.dataset.rm, 1)
      renderCredList()
    })
  })
}
function st_cls(s) {
  return s === 'ok' ? 'ok' : s === 'err' ? 'err' : 'working'
}
function openCred() {
  document.getElementById('cred-overlay').hidden = false
  if (importSnap && (importSnap.running || importActive(importSnap))) {
    ensureImportPolling()
    renderImportProgress(importSnap)
    return
  }
  credEnterEditMode()
}
function credEnterEditMode() {
  credList = []
  document.getElementById('cred-file-input').value = ''
  const prog = document.getElementById('cred-progress')
  prog.hidden = true
  document.getElementById('cred-progress-fill').style.width = '0'
  document.getElementById('cred-apply').hidden = false
  document.getElementById('cred-apply').disabled = false
  document.getElementById('cred-cancel').hidden = true
  document.getElementById('cred-newlist').hidden = true
  document.getElementById('cred-add').disabled = false
  document.getElementById('cred-drop').style.pointerEvents = ''
  renderCredList()
}
function closeCred() {
  document.getElementById('cred-overlay').hidden = true
}
function setPassReveal(on) {
  const wrap = document.getElementById('credentry-passwrap')
  const inp = document.getElementById('credentry-pass')
  wrap.classList.toggle('revealed', on)
  inp.type = on ? 'text' : 'password'
}
function openCredEntry(index) {
  credEntryIndex = index === undefined ? -1 : index
  const existing = credEntryIndex >= 0 ? credList[credEntryIndex] : null
  document.getElementById('credentry-title').textContent = existing ? 'Изменить аккаунт' : 'Новый аккаунт'
  document.getElementById('credentry-login').value = existing ? existing.login : ''
  document.getElementById('credentry-pass').value = existing ? existing.password : ''
  setPassReveal(false)
  document.getElementById('credentry-overlay').hidden = false
  setTimeout(() => document.getElementById('credentry-login').focus(), 30)
}
function closeCredEntry() {
  document.getElementById('credentry-overlay').hidden = true
}
function saveCredEntry() {
  const login = document.getElementById('credentry-login').value.trim()
  const password = document.getElementById('credentry-pass').value
  if (!login || !password) {
    toast('Укажи логин и пароль', true)
    return
  }
  const entry = { login, password }
  if (credEntryIndex >= 0) credList[credEntryIndex] = entry
  else credList.push(entry)
  dedupCred()
  renderCredList()
  closeCredEntry()
}
function parseCredTxt(text) {
  return (text || '')
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((line) => {
      const i = line.indexOf(':')
      if (i <= 0) return null
      return { login: line.slice(0, i).trim(), password: line.slice(i + 1) }
    })
    .filter((x) => x && x.login && x.password)
}
function dedupCred() {
  const seen = new Set()
  const before = credList.length
  credList = credList.filter((c) => {
    const k = (c.login || '').trim().toLowerCase()
    if (!k || seen.has(k)) return false
    seen.add(k)
    return true
  })
  return before - credList.length
}
function ingestCredTxt(fileList) {
  const file = (fileList || [])[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = () => {
    const parsed = parseCredTxt(String(reader.result || ''))
    if (!parsed.length) {
      toast('В файле нет строк формата login:pass', true)
      return
    }
    parsed.forEach((p) => credList.push({ login: p.login, password: p.password }))
    const removed = dedupCred()
    renderCredList()
    toast('Добавлено: ' + (parsed.length - removed) + (removed ? ' (дублей убрано: ' + removed + ')' : ''))
  }
  reader.readAsText(file)
}
document.getElementById('cred-btn').addEventListener('click', openCred)
document.getElementById('cred-close').addEventListener('click', closeCred)
document.getElementById('cred-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'cred-overlay') closeCred()
})
document.getElementById('cred-add').addEventListener('click', () => openCredEntry())
document.getElementById('credentry-close').addEventListener('click', closeCredEntry)
document.getElementById('credentry-cancel').addEventListener('click', closeCredEntry)
document.getElementById('credentry-save').addEventListener('click', saveCredEntry)
document.getElementById('credentry-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'credentry-overlay') closeCredEntry()
})
document.getElementById('credentry-eye').addEventListener('click', () => {
  const on = !document.getElementById('credentry-passwrap').classList.contains('revealed')
  setPassReveal(on)
})
document.getElementById('credentry-pass').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') saveCredEntry()
})
const credDrop = document.getElementById('cred-drop')
const credInput = document.getElementById('cred-file-input')
credDrop.addEventListener('click', () => credInput.click())
credInput.addEventListener('change', (e) => {
  ingestCredTxt(e.target.files)
  e.target.value = ''
})
credDrop.addEventListener('dragover', (e) => {
  e.preventDefault()
  credDrop.classList.add('drag')
})
credDrop.addEventListener('dragleave', () => credDrop.classList.remove('drag'))
credDrop.addEventListener('drop', (e) => {
  e.preventDefault()
  credDrop.classList.remove('drag')
  ingestCredTxt(e.dataTransfer && e.dataTransfer.files)
})
document.getElementById('cred-apply').addEventListener('click', async () => {
  if (!credList.length) {
    toast('Добавь хотя бы один аккаунт', true)
    return
  }
  const accounts = credList.map((c) => ({ login: c.login, password: c.password }))
  const applyBtn = document.getElementById('cred-apply')
  applyBtn.disabled = true
  importCancelling = false
  try {
    const r = await apiPost('/api/accounts/import-start', { accounts })
    if (!r.ok) {
      applyBtn.disabled = false
      toast(r.error || 'не удалось запустить импорт', true)
      return
    }
  } catch (e) {
    applyBtn.disabled = false
    toast('не удалось запустить импорт', true)
    return
  }
  toast('Импорт запущен - можно закрыть окно, продолжит в фоне')
  ensureImportPolling()
  await pollImport()
})
let importCancelling = false
document.getElementById('cred-cancel').addEventListener('click', async () => {
  if (importCancelling) return
  importCancelling = true
  const btn = document.getElementById('cred-cancel')
  btn.disabled = true
  btn.textContent = 'Отменяю…'
  toast('Отмена импорта…')
  try {
    await apiPost('/api/accounts/import-cancel', {})
  } catch (e) {
    /* ignore */
  }
})
document.getElementById('cred-newlist').addEventListener('click', () => {
  importSnap = null
  credEnterEditMode()
})
document.getElementById('prompt-ok').addEventListener('click', () => closePrompt(document.getElementById('prompt-input').value))
document.getElementById('prompt-cancel').addEventListener('click', () => closePrompt(null))
document.getElementById('prompt-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'prompt-overlay') closePrompt(null)
})
document.getElementById('prompt-input').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') closePrompt(document.getElementById('prompt-input').value)
  if (e.key === 'Escape') closePrompt(null)
})
document.getElementById('confirm-yes').addEventListener('click', () => closeConfirm(true))
document.getElementById('confirm-no').addEventListener('click', () => closeConfirm(false))
document.getElementById('confirm-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'confirm-overlay') closeConfirm(false)
})

async function refreshPlayerInfo() {
  const names = new Set()
  state.accounts.forEach((a) => {
    if (a.name) names.add(a.name)
  })
  for (const name of names) {
    const key = name.toLowerCase()
    try {
      const info = await apiGet('/api/player?name=' + encodeURIComponent(name))
      if (info && info.name) state.playerInfo.set(key, info)
      else if (!state.playerInfo.get(key)) state.playerInfo.set(key, null)
    } catch (e) {
      /* keep previous */
    }
  }
  state.cards.forEach((card) => {
    const acc = cardAccount(card)
    if (acc) applyPlayerInfo(card, acc)
  })
  if (state.selected) {
    renderModalGroups(state.selected)
    renderModalStats(state.selected)
  }
}

state.groupProfileDD = initDropdown('group-profile-select', async (value) => {
  const group = activeGroup()
  if (!group) return
  try {
    await apiPost('/api/groups/profile', { id: group.id, profile: value })
    group.profile = value
    toast(value ? 'Профиль группы: ' + value : 'Профиль группы убран')
  } catch (err) {
    toast(err.message, true)
  }
})

document.getElementById('group-trigger').addEventListener('click', (e) => {
  e.stopPropagation()
  const dd = document.getElementById('group-dd')
  if (dd.classList.contains('open')) closeGroupMenu()
  else openGroupMenu()
})
document.addEventListener('click', (e) => {
  if (!e.target.closest('#group-dd')) closeGroupMenu()
})
document.getElementById('filter-trigger').addEventListener('click', (e) => {
  e.stopPropagation()
  const dd = document.getElementById('filter-dd')
  if (dd.classList.contains('open')) closeFilterPanel()
  else openFilterPanel()
})
document.getElementById('filter-panel').addEventListener('click', (e) => e.stopPropagation())
document.addEventListener('click', (e) => {
  if (!e.target.closest('#filter-dd')) closeFilterPanel()
})
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return
  let closed = false
  const gdd = document.getElementById('group-dd')
  if (gdd && gdd.classList.contains('open')) {
    closeGroupMenu()
    closed = true
  }
  const fdd = document.getElementById('filter-dd')
  if (fdd && fdd.classList.contains('open')) {
    closeFilterPanel()
    closed = true
  }
  const ctx = document.getElementById('ctx-menu')
  if (ctx && !ctx.hidden) {
    closeCardMenu()
    closed = true
  }
  const bell = document.querySelector('.bell-wrap.open')
  if (bell) {
    bell.classList.remove('open')
    document.getElementById('notif-panel').hidden = true
    closed = true
  }
  if (document.querySelector('.ac-dd.open')) {
    closeAllDropdowns()
    closed = true
  }
  if (closed) return
  const modals = [
    ['confirm-overlay', () => closeConfirm(false)],
    ['prompt-overlay', () => closePrompt(null)],
    ['ram-overlay', closeRamModal],
    ['addmembers-overlay', closeAddMembers],
    ['settings-overlay', closeSettings],
    ['logs-overlay', closeLogs],
    ['groups-overlay', closeGroupsManage],
    ['credentry-overlay', closeCredEntry],
    ['cred-overlay', closeCred],
    ['import-overlay', closeImport],
    ['profiles-overlay', closeProfiles],
    ['overlay', closeModal],
  ]
  for (const [id, fn] of modals) {
    const el = document.getElementById(id)
    if (el && !el.hidden) {
      fn()
      return
    }
  }
})
document.getElementById('group-launch').addEventListener('click', launchActiveGroup)
document.getElementById('group-add').addEventListener('click', () => openAddMembers())
document.getElementById('group-add-center').addEventListener('click', () => openAddMembers())
document.getElementById('group-ram').addEventListener('click', openRamModal)

const mRamSlider = document.getElementById('modal-ram-slider')
const mRamInput = document.getElementById('modal-ram-input')
mRamSlider.addEventListener('input', () => {
  mRamInput.value = mRamSlider.value
})
mRamSlider.addEventListener('change', () => saveModalRam(mRamSlider.value))
mRamInput.addEventListener('change', () => saveModalRam(mRamInput.value))
bindRamSteppers(mRamInput, mRamSlider, saveModalRam)

const gRamSlider = document.getElementById('ram-modal-slider')
const gRamInput = document.getElementById('ram-modal-input')
gRamSlider.addEventListener('input', () => {
  gRamInput.value = gRamSlider.value
})
gRamInput.addEventListener('change', () => {
  const v = clampRam(gRamInput.value)
  gRamInput.value = v
  gRamSlider.value = v
})
bindRamSteppers(gRamInput, gRamSlider, null)
document.getElementById('ram-close').addEventListener('click', closeRamModal)
document.getElementById('ram-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'ram-overlay') closeRamModal()
})
document.getElementById('ram-select-all').addEventListener('click', () => {
  const all = state.ramMembers && state.ramPick && state.ramPick.size === state.ramMembers.length
  state.ramPick = new Set(all ? [] : (state.ramMembers || []).map((m) => m.uuid))
  renderRamPick()
})
document.getElementById('ram-apply').addEventListener('click', applyGroupRam)
document.getElementById('addmembers-close').addEventListener('click', closeAddMembers)
document.getElementById('addmembers-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'addmembers-overlay') closeAddMembers()
})
document.getElementById('addmembers-search').addEventListener('input', renderAddMembers)
document.addEventListener('keydown', (e) => {
  if (e.ctrlKey && e.shiftKey && (e.key === 'E' || e.key === 'e')) {
    state.debugExpired = !state.debugExpired
    state.cards.forEach((card, uuid) => {
      const acc = state.byUuid.get(uuid)
      if (acc) fillCard(card, acc)
    })
    toast(state.debugExpired ? 'Тест: все токены «истёк»' : 'Тест выключен')
  }
})
document.addEventListener('click', () => closeCardMenu())
document.addEventListener('scroll', () => closeCardMenu(), true)
window.addEventListener('resize', () => closeCardMenu())
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeCardMenu()
})
document.addEventListener('contextmenu', (e) => {
  if (!e.target.closest('.acc-card')) closeCardMenu()
})
function setToggle(id, on) {
  const t = document.getElementById(id)
  t.classList.toggle('on', on)
  t.setAttribute('aria-checked', on ? 'true' : 'false')
}
const LAUNCHER_LABELS = { normal: 'Обычный лаунчер', jar: 'JAR-версия лаунчера', new: 'Новый лаунчер' }
let currentLauncher = 'jar'
function setLauncherUI(v) {
  currentLauncher = LAUNCHER_LABELS[v] ? v : 'jar'
  document.getElementById('launcher-dd-value').textContent = LAUNCHER_LABELS[currentLauncher]
  document.querySelectorAll('#launcher-dd-menu .launcher-dd-item').forEach((el) => {
    el.classList.toggle('on', el.dataset.val === currentLauncher)
  })
}
function toggleLauncherMenu(force) {
  const dd = document.getElementById('launcher-dd')
  const menu = document.getElementById('launcher-dd-menu')
  const open = force !== undefined ? force : menu.hidden
  menu.hidden = !open
  dd.classList.toggle('open', open)
}

async function openSettings() {
  document.getElementById('settings-overlay').hidden = false
  setToggle('toggle-sound', window.soundEnabled ? window.soundEnabled() : true)
  try {
    const s = await apiGet('/api/settings')
    setToggle('toggle-autostart', !!s.autostart)
    setToggle('toggle-autoplay', s.autoPlay !== false)
    setToggle('toggle-stats', s.stats !== false)
    setLauncherUI(s.launcher)
  } catch (e) {
    /* ignore */
  }
}
function closeSettings() {
  toggleLauncherMenu(false)
  document.getElementById('settings-overlay').hidden = true
}
function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme)
  const btn = document.getElementById('theme-btn')
  btn.title = theme === 'dark' ? 'Светлая тема' : 'Тёмная тема'
  const dark = theme === 'dark'
  const sun = btn.querySelector('.ic-sun')
  const moon = btn.querySelector('.ic-moon')
  sun.style.opacity = dark ? '1' : '0'
  sun.style.transform = dark ? 'rotate(0) scale(1)' : 'rotate(90deg) scale(0.4)'
  moon.style.opacity = dark ? '0' : '1'
  moon.style.transform = dark ? 'rotate(-90deg) scale(0.4)' : 'rotate(0) scale(1)'
}
let currentTheme = localStorage.getItem('ac-theme') === 'light' ? 'light' : 'dark'
applyTheme(currentTheme)
document.getElementById('theme-btn').addEventListener('click', () => {
  currentTheme = currentTheme === 'light' ? 'dark' : 'light'
  localStorage.setItem('ac-theme', currentTheme)
  applyTheme(currentTheme)
  playSound('theme', currentTheme === 'light')
})

document.querySelectorAll('.rz').forEach((h) => {
  h.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return
    e.preventDefault()
    if (window.acResize) window.acResize(+h.dataset.rz)
  })
})

window.addEventListener('dragover', (e) => e.preventDefault())
window.addEventListener('drop', (e) => e.preventDefault())

document.addEventListener(
  'click',
  (e) => {
    const b = e.target.closest('button')
    if (!b || b.disabled) return
    if (b.id === 'theme-btn') return
    if (b.id === 'bell-btn') return playSound('bell')
    if (b.id === 'settings-btn') return playSound('gear')
    if (b.id === 'cred-btn') return playSound('confirm')
    if (b.id === 'refresh-tokens-btn') return playSound('confirm')
    if (b.classList.contains('primary')) return playSound('confirm')
    playSound('tap')
  },
  true,
)
document.getElementById('settings-btn').addEventListener('click', openSettings)
document.getElementById('settings-close').addEventListener('click', closeSettings)
document.getElementById('settings-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'settings-overlay') closeSettings()
})
document.getElementById('toggle-autostart').addEventListener('click', async () => {
  const next = !document.getElementById('toggle-autostart').classList.contains('on')
  setToggle('toggle-autostart', next)
  try {
    const r = await apiPost('/api/settings/autostart', { enabled: next })
    setToggle('toggle-autostart', !!r.autostart)
    toast(r.autostart ? 'Автозапуск включён' : 'Автозапуск выключен')
  } catch (e) {
    setToggle('toggle-autostart', !next)
    toast(e.message, true)
  }
})
document.getElementById('toggle-sound').addEventListener('click', () => {
  const next = !document.getElementById('toggle-sound').classList.contains('on')
  window.setSoundEnabled(next)
  setToggle('toggle-sound', next)
  if (next) playSound('tap')
  toast(next ? 'Звуки включены' : 'Звуки выключены')
})
document.getElementById('toggle-autoplay').addEventListener('click', async () => {
  const next = !document.getElementById('toggle-autoplay').classList.contains('on')
  setToggle('toggle-autoplay', next)
  try {
    const r = await apiPost('/api/settings/autoplay', { enabled: next })
    setToggle('toggle-autoplay', r.autoPlay !== false)
    toast(next ? 'Автонажатие ИГРАТЬ включено' : 'Автонажатие ИГРАТЬ выключено')
  } catch (e) {
    setToggle('toggle-autoplay', !next)
    toast(e.message, true)
  }
})
document.getElementById('toggle-stats').addEventListener('click', async () => {
  const next = !document.getElementById('toggle-stats').classList.contains('on')
  setToggle('toggle-stats', next)
  try {
    const r = await apiPost('/api/settings/stats', { enabled: next })
    setToggle('toggle-stats', r.stats !== false)
    pollActive()
    toast(next ? 'Отправка метрик включена' : 'Отправка метрик выключена')
  } catch (e) {
    setToggle('toggle-stats', !next)
    toast(e.message, true)
  }
})
document.getElementById('launcher-dd-trigger').addEventListener('click', (e) => {
  e.stopPropagation()
  toggleLauncherMenu()
})
document.querySelectorAll('#launcher-dd-menu .launcher-dd-item').forEach((el) => {
  el.addEventListener('click', async () => {
    const v = el.dataset.val
    toggleLauncherMenu(false)
    if (v === currentLauncher) return
    const prev = currentLauncher
    setLauncherUI(v)
    try {
      const r = await apiPost('/api/settings/launcher', { launcher: v })
      setLauncherUI(r.launcher)
      toast('Лаунчер: ' + LAUNCHER_LABELS[r.launcher])
    } catch (err) {
      setLauncherUI(prev)
      toast(err.message, true)
    }
  })
})
document.addEventListener('click', () => toggleLauncherMenu(false))
document.getElementById('logs-btn').addEventListener('click', openLogs)
document.getElementById('logs-close').addEventListener('click', closeLogs)
document.getElementById('logs-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'logs-overlay') closeLogs()
})
document.getElementById('logs-clear').addEventListener('click', async () => {
  if (!logsState.uuid) return
  await apiPost('/api/logs/clear', { uuid: logsState.uuid })
  logsState.uuid = null
  document.getElementById('logs-current').textContent = 'Выбери аккаунт слева'
  document.getElementById('logs-clear').hidden = true
  document.getElementById('logs-view').textContent = ''
  refreshLogsSide()
})
document.getElementById('groups-manage-list').addEventListener('dragover', groupDragOver)
document.getElementById('groups-close').addEventListener('click', closeGroupsManage)
document.getElementById('groups-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'groups-overlay') closeGroupsManage()
})
document.getElementById('groups-create').addEventListener('click', async () => {
  await createGroupFlow()
  renderGroupsManage()
})

let statsSnap = null
let statsOff = false
async function pollActive() {
  const badge = document.getElementById('active-badge')
  try {
    const s = await apiGet('/api/stats')
    badge.hidden = false
    if (s && s.enabled === false) {
      statsSnap = null
      statsOff = true
      badge.classList.add('off')
      badge.setAttribute('data-tip', 'Отправка метрик выключена, потому онлайн скрыт')
      document.getElementById('active-count').textContent = '0'
    } else if (s && typeof s.online === 'number' && s.online >= 0) {
      statsSnap = s
      statsOff = false
      badge.classList.remove('off')
      badge.setAttribute('data-tip', 'Сейчас онлайн - нажми для подробностей')
      document.getElementById('active-count').textContent = s.online
    } else {
      statsSnap = null
      statsOff = false
      badge.classList.add('off')
      badge.setAttribute('data-tip', 'Онлайн временно недоступен')
      document.getElementById('active-count').textContent = '0'
    }
    if (!document.getElementById('online-pop').hidden) fillOnlinePop()
  } catch (e) {
    /* ignore */
  }
}
function fillOnlinePop() {
  const rows = document.getElementById('online-pop-rows')
  const note = document.getElementById('online-pop-note')
  if (statsSnap) {
    document.getElementById('op-online').textContent = statsSnap.online
    document.getElementById('op-active').textContent = statsSnap.active
    document.getElementById('op-total').textContent = statsSnap.total
    rows.hidden = false
    note.hidden = true
  } else {
    note.textContent = statsOff
      ? 'Вы отключили отправку метрик - онлайн недоступен. Включить можно в настройках.'
      : 'Онлайн временно недоступен.'
    rows.hidden = true
    note.hidden = false
  }
}
document.body.appendChild(document.getElementById('online-pop'))
document.getElementById('active-badge').addEventListener('click', (e) => {
  e.stopPropagation()
  playSound('tap')
  const pop = document.getElementById('online-pop')
  if (!pop.hidden) {
    pop.hidden = true
    return
  }
  fillOnlinePop()
  pop.hidden = false
  const br = document.getElementById('active-badge').getBoundingClientRect()
  const left = Math.min(br.left, window.innerWidth - pop.offsetWidth - 8)
  pop.style.left = Math.max(8, left) + 'px'
  pop.style.top = br.bottom + 8 + 'px'
})
document.addEventListener('click', (e) => {
  const pop = document.getElementById('online-pop')
  if (pop.hidden) return
  if (!pop.contains(e.target) && !document.getElementById('active-badge').contains(e.target)) {
    pop.hidden = true
  }
})

loadProfiles().then(() => loadGroups())
loadAccounts()
checkUpdate()
pollActive()
pollImport().then(() => {
  if (importSnap && importSnap.running) ensureImportPolling()
})
setInterval(loadAccounts, 4000)
setInterval(loadGroups, 8000)
setInterval(refreshPlayerInfo, 60000)
setInterval(checkUpdate, 30 * 60 * 1000)
setInterval(() => {
  if (!statsOff) pollActive()
}, 60 * 1000)

;(function () {
  const tip = document.createElement('div')
  tip.className = 'tooltip'
  tip.hidden = true
  document.body.appendChild(tip)
  let cur = null
  function place(el) {
    const r = el.getBoundingClientRect()
    const tr = tip.getBoundingClientRect()
    let left = r.left + r.width / 2 - tr.width / 2
    left = Math.max(6, Math.min(left, window.innerWidth - tr.width - 6))
    let top = r.top - tr.height - 8
    if (top < 6) top = r.bottom + 8
    tip.style.left = left + 'px'
    tip.style.top = top + 'px'
  }
  function show(el) {
    if (el.id === 'active-badge' && !document.getElementById('online-pop').hidden) return
    let text = el.getAttribute('data-tip')
    if (text == null && el.hasAttribute('title')) {
      text = el.getAttribute('title')
      el.setAttribute('data-tip', text)
      el.removeAttribute('title')
    }
    if (!text) return
    cur = el
    tip.textContent = text
    tip.hidden = false
    place(el)
  }
  function hide() {
    if (!cur) return
    cur = null
    tip.hidden = true
  }
  document.addEventListener('mouseover', (e) => {
    const el = e.target.closest('[title],[data-tip]')
    if (el && el !== cur) show(el)
  })
  document.addEventListener('mouseout', (e) => {
    if (cur && !cur.contains(e.relatedTarget)) hide()
  })
  document.addEventListener('mousedown', hide)
  window.addEventListener('scroll', hide, true)
})()
