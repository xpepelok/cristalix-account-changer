function clampRam(v) {
  return Math.max(1024, Math.min(32768, Math.round((+v || 1024) / 512) * 512))
}

function ramIsAuto(acc) {
  return !acc || !(acc.ram >= 512)
}

function paintModalRamAuto(auto) {
  document.getElementById('modal-ram-auto').classList.toggle('on', auto)
  document.getElementById('modal-ram-auto').setAttribute('aria-checked', auto ? 'true' : 'false')
  document.querySelector('.ram-row').classList.toggle('is-auto', auto)
}

function setModalRam(acc) {
  const v = acc && acc.ram >= 512 ? acc.ram : 2048
  document.getElementById('modal-ram-slider').value = v
  document.getElementById('modal-ram-input').value = v
  paintModalRamAuto(ramIsAuto(acc))
}

async function saveModalRamValue(ram) {
  if (!state.selected) return
  try {
    await apiPost('/api/account/ram', { uuids: [state.selected.uuid], ram })
    state.selected.ram = ram
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
}

async function toggleModalRamAuto() {
  if (!state.selected) return
  const auto = !ramIsAuto(state.selected)
  paintModalRamAuto(auto)
  if (auto) {
    await saveModalRamValue(0)
    return
  }
  const ram = clampRam(document.getElementById('modal-ram-input').value)
  document.getElementById('modal-ram-slider').value = ram
  document.getElementById('modal-ram-input').value = ram
  await saveModalRamValue(ram)
}

async function saveModalRam(v) {
  if (!state.selected) return
  const ram = clampRam(v)
  document.getElementById('modal-ram-slider').value = ram
  document.getElementById('modal-ram-input').value = ram
  paintModalRamAuto(false)
  try {
    await apiPost('/api/account/ram', { uuids: [state.selected.uuid], ram })
    state.selected.ram = ram
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
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
  const box = document.getElementById('modal-stats')
  if (lastModalStats === tiles) return
  lastModalStats = tiles
  box.innerHTML = tiles
}

function openModal(acc) {
  state.selected = acc
  document.getElementById('modal-name').textContent = acc.name || shortId(acc.uuid) + '…'
  document.getElementById('modal-uuid').textContent = acc.uuid
  document.getElementById('modal-pin').classList.toggle('active', !!acc.pinned)
  document.getElementById('modal-pin').title = acc.pinned ? 'Открепить' : 'Закрепить'
  document.getElementById('modal-label').value = acc.label || ''
  fillProfileSelect(acc)
  loadProfiles().then(() => {
    if (state.selected === acc) fillProfileSelect(acc)
  })
  setModalRam(acc)
  renderModalGroups(acc)
  renderModalStats(acc)
  if (acc.name) ensurePlayerInfo(acc.name)

  paintModalState(acc)

  overlay.hidden = false
  mountSkin(acc.uuid)
}

function paintModalState(acc) {
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
  document.getElementById('modal-pin').classList.toggle('active', !!acc.pinned)
  document.getElementById('modal-pin').title = acc.pinned ? 'Открепить' : 'Закрепить'
}

function refreshOpenModal() {
  if (!state.selected) return
  if (overlay.hidden) return
  const fresh = state.byUuid.get(state.selected.uuid)
  if (!fresh) return
  state.selected = fresh
  paintModalState(fresh)
  renderModalStats(fresh)
  renderModalGroups(fresh)
}

function closeModal() {
  overlay.hidden = true
  disposeSkin()
  state.selected = null
}
