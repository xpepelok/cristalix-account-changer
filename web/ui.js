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

function closeConfirm(value) {
  document.getElementById('confirm-overlay').hidden = true
  if (confirmResolve) {
    const r = confirmResolve
    confirmResolve = null
    r(value)
  }
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

function formatDurationSecs(secs) {
  secs = Math.max(0, Math.floor(secs))
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = secs % 60
  if (h) return h + ' ч ' + String(m).padStart(2, '0') + ' мин'
  if (m) return m + ' мин ' + String(s).padStart(2, '0') + ' с'
  return s + ' с'
}

function personTileHTML(uuid, name, label) {
  if (!name) return '<div class="stat-tile"><div class="stat-tile-value">—</div><div class="stat-tile-label">' + esc(label) + '</div></div>'
  const info = state.playerInfo.get(name.toLowerCase())
  const grp = primaryGroup(info)
  const chip = grp ? '<span class="grp-chip" style="--chip:' + esc(grp.color) + '">' + esc(grp.label) + '</span>' : ''
  return (
    '<div class="stat-tile"><div class="stat-tile-person-row"><span class="avatar-wrap"><span class="avatar mini" style="background-image:' +
    skinBg(uuid) +
    '"><span class="avatar-hat" style="background-image:' +
    skinBg(uuid) +
    '"></span></span></span>' +
    chip +
    '<span class="stat-tile-person-name">' +
    esc(name) +
    '</span>' +
    '</div><div class="stat-tile-label">' +
    esc(label) +
    '</div></div>'
  )
}

async function openStats() {
  document.getElementById('stats-overlay').hidden = false
  const grid = document.getElementById('stats-grid')
  grid.innerHTML = ''
  try {
    const st = await apiGet('/api/stats/logs')
    if (st.mostLaunchedName) await ensurePlayerInfo(st.mostLaunchedName)
    if (st.longestName) await ensurePlayerInfo(st.longestName)
    const tiles = [
      { html: '<div class="stat-tile"><div class="stat-tile-value">' + esc(String(st.totalAccounts)) + '</div><div class="stat-tile-label">Аккаунтов в логах</div></div>' },
      { html: '<div class="stat-tile"><div class="stat-tile-value">' + esc(String(st.totalSessions)) + '</div><div class="stat-tile-label">Всего сессий запуска</div></div>' },
      { html: '<div class="stat-tile"><div class="stat-tile-value' + (st.activeNow ? ' green' : '') + '">' + esc(String(st.activeNow)) + '</div><div class="stat-tile-label">Активно сейчас</div></div>' },
      { html: '<div class="stat-tile"><div class="stat-tile-value">' + esc(formatDurationSecs(st.avgDuration)) + '</div><div class="stat-tile-label">Среднее время сессии</div></div>' },
      { html: '<div class="stat-tile"><div class="stat-tile-value">' + esc(formatDurationSecs(st.totalDuration)) + '</div><div class="stat-tile-label">Суммарное время сессий</div></div>' },
      { html: personTileHTML(st.longestUuid, st.longestName, 'Самая долгая сессия (' + formatDurationSecs(st.longestDuration) + ')') },
      { html: personTileHTML(st.mostLaunchedUuid, st.mostLaunchedName, 'Чаще всего запускался') },
      { html: '<div class="stat-tile"><div class="stat-tile-value">' + esc(String(st.totalLines)) + '</div><div class="stat-tile-label">Строк логов записано</div></div>' },
    ]
    grid.innerHTML = tiles.map((t) => t.html).join('')
  } catch (e) {
    grid.innerHTML = '<div class="stat-tile"><div class="stat-tile-value">—</div><div class="stat-tile-label">Не удалось загрузить</div></div>'
  }
}

function closeStats() {
  document.getElementById('stats-overlay').hidden = true
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

function openUrl(url) {
  if (typeof window.acOpenUrl === 'function') {
    window.acOpenUrl(url)
  } else {
    window.open(url, '_blank')
  }
}

function setBellNotif(active) {
  document.getElementById('bell-dot').hidden = !active
  document.getElementById('bell-btn').classList.toggle('has-notif', active)
}

function fmtBytes(n) {
  if (!n || n < 1024) return (n || 0) + ' Б'
  if (n < 1024 * 1024) return (n / 1024).toFixed(0) + ' КБ'
  return (n / (1024 * 1024)).toFixed(1) + ' МБ'
}

function st_cls(s) {
  return s === 'ok' ? 'ok' : s === 'err' ? 'err' : 'working'
}

function setPassReveal(on) {
  const wrap = document.getElementById('credentry-passwrap')
  const inp = document.getElementById('credentry-pass')
  wrap.classList.toggle('revealed', on)
  inp.type = on ? 'text' : 'password'
}

function setLauncherUI(v) {
  currentLauncher = LAUNCHER_LABELS[v] ? v : 'jar'
  document.getElementById('launcher-dd-value').textContent = LAUNCHER_LABELS[currentLauncher]
  document.querySelectorAll('#launcher-dd-menu .launcher-dd-item').forEach((el) => {
    el.classList.toggle('on', el.dataset.val === currentLauncher)
  })
  document.getElementById('custom-launcher-row').hidden = currentLauncher !== 'custom'
}

async function pickCustomLauncher() {
  if (typeof window.acPickLauncher !== 'function') {
    toast('Выбор файла доступен только в приложении', true)
    return
  }
  const path = await window.acPickLauncher()
  if (!path) return
  try {
    const r = await apiPost('/api/settings/custom-launcher', { path })
    customLauncherPath = r.customLauncher || ''
    updateCustomPathUI()
    setLauncherUI(r.launcher)
    toast('Свой лаунчер: ' + baseName(customLauncherPath))
  } catch (err) {
    toast(err.message, true)
  }
}

function baseName(path) {
  return String(path || '').split(/[\\/]/).pop()
}

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
