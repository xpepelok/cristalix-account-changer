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
    { uuids, ram, renderDistance: chunks, maxFps: fps, animations: state.lsClient.animations, fastRender: state.lsClient.fastRender, minimal: !!state.lsMinimal },
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

function renderModalGroups(acc) {
  const box = document.getElementById('modal-groups')
  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  if (!info) {
    if (lastModalGroups === '') return
    lastModalGroups = ''
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
  const html = tags
    .map((t) => `<span class="grp-tag" style="--chip:${t.color}">${esc(t.label)}</span>`)
    .join('')
  if (lastModalGroups === html) return
  lastModalGroups = html
  box.innerHTML = html
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
    pollGroupLaunchProgress()
  } catch (e) {
    toast(e.message, true)
  }
}

function renderGroupLaunchButton(p) {
  const btn = document.getElementById('group-launch')
  const label = btn.querySelector('.group-launch-label')
  const playIcon = btn.querySelector('.group-launch-play')

  if (!p || !p.active) {
    btn.classList.remove('launching', 'paused')
    playIcon.style.display = ''
    label.textContent = 'Запустить все'
    btn.title = 'Запустить все аккаунты группы поочерёдно'
    return
  }

  btn.classList.add('launching')
  btn.classList.toggle('paused', p.paused)
  playIcon.style.display = 'none'
  if (p.paused) {
    label.textContent = 'Возобновить'
    btn.title = 'Возобновить запуск аккаунтов'
  } else {
    label.textContent = 'Остановить'
    btn.title = 'Остановить запуск аккаунтов'
  }
}

async function pollGroupLaunchProgress() {
  try {
    const p = await apiGet('/api/groups/launch/progress')
    state.groupLaunchProgress = p
    renderGroupLaunchButton(p)
  } catch (e) {
    /* ignore */
  }
}

async function closeAllGroupAccounts() {
  const group = activeGroup()
  if (!group) return
  if (!(await confirmDialog('Закрыть все аккаунты?', 'Будут закрыты все запущенные на данный момент аккаунты группы «' + esc(group.name) + '».', 'Да, закрыть'))) return
  try {
    const res = await apiPost('/api/groups/close-all', { id: group.id })
    toast(res.closed ? 'Закрыто аккаунтов: ' + res.closed : 'Запущенных аккаунтов не было')
    loadAccounts()
    pollGroupLaunchProgress()
  } catch (e) {
    toast(e.message, true)
  }
}

async function toggleGroupLaunchPause() {
  const p = state.groupLaunchProgress
  if (!p || !p.active) return
  try {
    if (p.paused) await apiPost('/api/groups/launch/resume', {})
    else await apiPost('/api/groups/launch/pause', {})
  } catch (e) {
    toast(e.message, true)
  }
  pollGroupLaunchProgress()
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

function openGroupsManage() {
  document.getElementById('groups-overlay').hidden = false
  renderGroupsManage()
}

function closeGroupsManage() {
  document.getElementById('groups-overlay').hidden = true
}

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
      '<button class="gm-pin' + (g.pinned ? ' on' : '') + '" data-act="pin" title="' + (g.pinned ? 'Открепить' : 'Закрепить') + '">' + STAR_SVG + '</button>' +
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
