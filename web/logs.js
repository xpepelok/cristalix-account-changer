function confirmDialog(title, text, yesLabel) {
  return new Promise((resolve) => {
    confirmResolve = resolve
    document.getElementById('confirm-title').textContent = title
    document.getElementById('confirm-text').innerHTML = text
    document.getElementById('confirm-yes').textContent = yesLabel || 'Да, удалить'
    document.getElementById('confirm-overlay').hidden = false
  })
}

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
  }, 1000)
  if (logsState.tick) clearInterval(logsState.tick)
  logsState.tick = setInterval(tickLogDuration, 250)
}

function tickLogDuration() {
  if (!logsState.open) return
  const session = logsState.current
  if (!session || !session.active) return
  const el = document.getElementById('logs-duration')
  if (!el) return
  const text = logSessionDuration(session)
  if (el.textContent !== text) el.textContent = text
}

function closeLogs() {
  logsState.open = false
  document.getElementById('logs-overlay').hidden = true
  if (logsState.timer) {
    clearInterval(logsState.timer)
    logsState.timer = null
  }
  if (logsState.tick) {
    clearInterval(logsState.tick)
    logsState.tick = null
  }
}

function resetLogsSelection() {
  logsState.uuid = null
  logsState.session = null
  document.getElementById('logs-current').textContent = 'Выбери аккаунт слева'
  document.getElementById('logs-sessions').hidden = true
  const meta = document.getElementById('logs-session-meta')
  meta.hidden = true
  meta.innerHTML = ''
  const view = document.getElementById('logs-view')
  view.classList.add('is-empty')
  view.textContent = 'Выбери аккаунт слева, чтобы увидеть его лог'
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
  document.getElementById('logs-clear').hidden = !logs.length
  if (!logs.length) {
    side.innerHTML = '<div class="logs-empty">Пока никого не запускали через ченджер</div>'
    if (logsState.uuid) resetLogsSelection()
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
  if (logsState.uuid && !logs.some((l) => l.uuid === logsState.uuid)) {
    resetLogsSelection()
  }
  if (!logsState.uuid && logs[0]) {
    selectLog(logs[0].uuid)
  }
}

function selectLog(uuid) {
  logsState.uuid = uuid
  logsState.session = null
  const acc = state.byUuid.get(uuid)
  document.getElementById('logs-current').textContent = acc ? acc.name || shortId(uuid) : shortId(uuid)
  document.querySelectorAll('#logs-side .logs-item').forEach((i) => i.classList.toggle('active', i.dataset.uuid === uuid))
  refreshLogsView(true)
}

function logSessionLabel(sessions, session) {
  if (session.active) return 'Текущая'
  const d = new Date(session.started * 1000)
  const key = d.toLocaleDateString('ru-RU')
  const number = sessions.filter((x) => new Date(x.started * 1000).toLocaleDateString('ru-RU') === key && x.started <= session.started).length
  return String(d.getDate()).padStart(2, '0') + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + d.getFullYear() + ' (' + number + ')'
}

function logSessionTime(timestamp) {
  if (!timestamp) return '—'
  const d = new Date(timestamp * 1000)
  const time = [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, '0')).join(':')
  const date = [d.getDate(), d.getMonth() + 1, d.getFullYear()].map((n) => String(n).padStart(2, '0')).join('-')
  return time + ' ' + date
}

function logSessionDuration(session) {
  if (!session.started) return '—'
  const end = session.active ? Date.now() / 1000 : session.ended || session.started
  let secs = Math.max(0, Math.floor(end - session.started))
  const h = Math.floor(secs / 3600)
  secs -= h * 3600
  const m = Math.floor(secs / 60)
  secs -= m * 60
  const parts = h ? [h, m, secs] : [m, secs]
  return parts.map((n) => String(n).padStart(2, '0')).join(':')
}

function renderLogSessions(sessions) {
  const holder = document.getElementById('logs-sessions')
  const meta = document.getElementById('logs-session-meta')
  holder.hidden = !sessions.length
  if (!sessions.length) {
    meta.hidden = true
    meta.innerHTML = ''
    return
  }
  if (!logsState.session || !sessions.some((s) => s.id === logsState.session)) logsState.session = sessions[0].id
  holder.innerHTML = sessions
    .map(
      (s) =>
        '<button class="log-session' +
        (s.id === logsState.session ? ' active' : '') +
        '" data-session="' +
        esc(s.id) +
        '" data-current="' +
        (s.active ? '1' : '0') +
        '">' +
        esc(logSessionLabel(sessions, s)) +
        '</button>',
    )
    .join('')
  holder.querySelectorAll('.log-session').forEach((b) => {
    b.addEventListener('click', () => {
      logsState.session = b.dataset.session
      refreshLogsView(true)
    })
    if (b.dataset.current === '1') return
    b.addEventListener('contextmenu', (e) => {
      e.preventDefault()
      openLogSessionMenu(e.clientX, e.clientY, b.dataset.session, b.textContent)
    })
  })
  const current = sessions.find((s) => s.id === logsState.session) || sessions[0]
  logsState.current = current
  meta.hidden = false
  const endDot = current.active ? 'pending' : 'end'
  const endValue = current.active ? 'Активна' : logSessionTime(current.ended)
  meta.innerHTML =
    '<div class="logs-session-meta-tile"><div class="logs-session-meta-value"><span class="logs-session-meta-dot start"></span><span class="logs-session-meta-text">' +
    esc(logSessionTime(current.started)) +
    '</span></div><div class="logs-session-meta-label">Начало сессии</div></div>' +
    '<div class="logs-session-meta-tile"><div class="logs-session-meta-value' +
    (current.active ? ' pending' : '') +
    '"><span class="logs-session-meta-dot duration"></span><span class="logs-session-meta-text" id="logs-duration">' +
    esc(logSessionDuration(current)) +
    '</span></div><div class="logs-session-meta-label">Длительность сессии</div></div>' +
    '<div class="logs-session-meta-tile"><div class="logs-session-meta-value' +
    (current.active ? ' pending' : '') +
    '"><span class="logs-session-meta-dot ' +
    endDot +
    '"></span><span class="logs-session-meta-text">' +
    esc(endValue) +
    '</span></div><div class="logs-session-meta-label">Конец сессии</div></div>'
}

function openLogSessionMenu(x, y, session, label) {
  const m = document.getElementById('ctx-menu')
  m.innerHTML = '<button class="ctx-item danger">Удалить сессию</button>'
  m.querySelector('button').addEventListener('click', async () => { closeCardMenu(); if (!await confirmDialog('Удалить сессию?', 'Вы действительно хотите удалить сессию «' + esc(label) + '»?')) return; await apiPost('/api/logs/session/delete', { uuid: logsState.uuid, session }); logsState.session = null; refreshLogsView(true); refreshLogsSide() })
  m.hidden = false; m.style.left = x + 'px'; m.style.top = y + 'px'; m.classList.add('show')
}

async function refreshLogsView(force) {
  if (!logsState.uuid) return
  let lines = []
  try {
    const suffix = logsState.session ? '&session=' + encodeURIComponent(logsState.session) : ''
    const data = await apiGet('/api/logs/get?uuid=' + encodeURIComponent(logsState.uuid) + suffix)
    lines = data.lines || []
    renderLogSessions(data.sessions || [])
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
