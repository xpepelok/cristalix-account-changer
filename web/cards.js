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
    if (!current) return
    playSound('tap')
    openModal(current)
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

function closeCardMenu() {
  const m = document.getElementById('ctx-menu')
  if (m.hidden) return
  m.classList.remove('show')
  m.hidden = true
  m.innerHTML = ''
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
  card._pin.title = acc.pinned ? 'Открепить' : 'Закрепить'
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

function renderLsClient() {
  const box = document.getElementById('ls-client')
  const seg = (key) => {
    const cur = state.lsClient[key]
    const opt = (v, t) => '<button class="ls-seg-btn' + (cur === v ? ' on' : '') + '" data-val="' + v + '">' + t + '</button>'
    return '<div class="ls-seg" data-key="' + key + '"><div class="ls-seg-thumb"></div>' + opt(0, 'Не менять') + opt(1, 'Выкл') + opt(2, 'Вкл') + '</div>'
  }
  box.innerHTML =
    '<div class="ls-client-row"><span class="ls-toggle-label">Минимальные настройки</span>' +
    '<button class="toggle' + (state.lsMinimal ? ' on' : '') + '" id="ls-minimal" role="switch"><span class="toggle-knob"></span></button></div>' +
    '<div class="ls-lockable" id="ls-lockable">' +
    '<div class="ls-client-row"><span class="ls-toggle-label">Прогрузка чанков</span>' +
    '<input type="number" class="ram-input ls-num" id="ls-chunks" min="2" max="32" step="1" value="' + (state.lsClient.renderDistance || '') + '"></div>' +
    '<div class="ls-client-row"><span class="ls-toggle-label">Макс. FPS</span>' +
    '<input type="number" class="ram-input ls-num" id="ls-fps" min="5" max="260" step="5" value="' + (state.lsClient.maxFps || '') + '"></div>' +
    LS_TRISTATE.map((t) => '<div class="ls-client-row"><span class="ls-toggle-label">' + t.label + '</span>' + seg(t.key) + '</div>').join('') +
    '</div>'
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
  const lockable = document.getElementById('ls-lockable')
  const applyLock = () => lockable.classList.toggle('ls-locked', state.lsMinimal)
  applyLock()
  document.getElementById('ls-minimal').addEventListener('click', () => {
    state.lsMinimal = !state.lsMinimal
    document.getElementById('ls-minimal').classList.toggle('on', state.lsMinimal)
    applyLock()
    if (state.lsMinimal) {
      const ramInput = document.getElementById('ram-modal-input')
      const ramSlider = document.getElementById('ram-modal-slider')
      ramInput.value = 1024
      ramSlider.value = 1024
      ramInput.dispatchEvent(new Event('change', { bubbles: true }))
    }
  })
}

function buildEditor(name, content, readonly) {
  const kvFiles = ['options.txt', 'optionsof.txt']
  const jsonFiles = ['binds.json', 'voicechat.json']
  const ro = readonly ? ' disabled' : ''
  state.editorFiles = {}
  let html = `<div class="editor-scroll"><div class="editor-head"><div class="editor-name">${esc(name)}</div></div>`
  for (const f of kvFiles) {
    const lines = parseLines(content[f] || '')
    state.editorFiles[f] = lines
    html += `<div class="editor-file" data-file="${f}"><div class="editor-file-title">${f}</div><div class="kv-rows">`
    const rows = lines
      .map((l, idx) =>
        l.kv
          ? `<div class="kv-row"><span class="kv-key" title="${esc(l.k)}">${esc(l.k)}</span><input class="kv-val" data-idx="${idx}" value="${esc(l.v)}"${ro} /></div>`
          : '',
      )
      .join('')
    html += rows || '<div class="editor-empty" style="padding:8px">файл пуст</div>'
    html += `</div></div>`
  }
  for (const f of jsonFiles) {
    html += `<div class="editor-file" data-file="${f}"><div class="editor-file-title">${f}</div><textarea class="json-area" data-file="${f}" spellcheck="false"${readonly ? ' readonly' : ''}>${esc(prettyJson(content[f] || ''))}</textarea></div>`
  }
  html += `</div>`
  html += readonly
    ? `<div class="editor-actions"><span class="editor-empty">Встроенный профиль — только для чтения</span></div>`
    : `<div class="editor-actions"><button class="action-btn primary" id="editor-save">Сохранить профиль</button><button class="action-btn danger-ghost" id="editor-delete">Удалить</button></div>`
  return html
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

function activeFilterCount() {
  return state.filters.groups.size + (state.filters.noRole ? 1 : 0) + (state.filters.expired ? 1 : 0) + (state.sort.key ? 1 : 0)
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

  refreshOpenModal()
  state.firstRender = false
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
