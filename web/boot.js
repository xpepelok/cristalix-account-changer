document.addEventListener('click', closeAllDropdowns)

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
  document.getElementById('modal-pin').title = next ? 'Открепить' : 'Закрепить'
  try {
    await apiPost('/api/pin', { uuid: state.selected.uuid, pinned: next })
    state.selected.pinned = next
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
})

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

document.getElementById('profile-import').addEventListener('click', openImport)

document.getElementById('import-close').addEventListener('click', closeImport)

document.getElementById('import-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'import-overlay') closeImport()
})

document.getElementById('import-name').addEventListener('input', renderImportFiles)

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

document.getElementById('group-launch').addEventListener('click', () => {
  const p = state.groupLaunchProgress
  if (!p || !p.active) {
    launchActiveGroup()
    return
  }
  toggleGroupLaunchPause()
})

document.getElementById('group-close-all').addEventListener('click', closeAllGroupAccounts)

setInterval(pollGroupLaunchProgress, 700)

document.getElementById('group-add').addEventListener('click', () => openAddMembers())

document.getElementById('group-add-center').addEventListener('click', () => openAddMembers())

document.getElementById('group-ram').addEventListener('click', openRamModal)

mRamSlider.addEventListener('input', () => {
  mRamInput.value = mRamSlider.value
})

mRamSlider.addEventListener('change', () => saveModalRam(mRamSlider.value))

mRamInput.addEventListener('change', () => saveModalRam(mRamInput.value))

bindRamSteppers(mRamInput, mRamSlider, saveModalRam)

document.getElementById('modal-ram-auto').addEventListener('click', toggleModalRamAuto)

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
  if (!e.target.closest('.acc-card') && !e.target.closest('.log-session')) closeCardMenu()
})

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
    if (b.id === 'stats-btn') return playSound('stats')
    if (b.id === 'logs-btn') return playSound('logs')
    if (b.id === 'profiles-btn') return playSound('profiles')
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

document.getElementById('toggle-aggressive').addEventListener('click', async () => {
  const next = !document.getElementById('toggle-aggressive').classList.contains('on')
  setToggle('toggle-aggressive', next)
  try {
    const r = await apiPost('/api/settings/aggressive', { enabled: next })
    setToggle('toggle-aggressive', !!r.aggressive)
    toast(next ? 'Агрессивный запуск включён' : 'Агрессивный запуск выключен')
  } catch (e) {
    setToggle('toggle-aggressive', !next)
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

document.getElementById('custom-launcher-browse').addEventListener('click', (e) => {
  e.stopPropagation()
  pickCustomLauncher()
})

document.querySelectorAll('#launcher-dd-menu .launcher-dd-item').forEach((el) => {
  el.addEventListener('click', async () => {
    const v = el.dataset.val
    toggleLauncherMenu(false)
    if (v === 'custom') {
      if (customLauncherPath) {
        const prev = currentLauncher
        setLauncherUI('custom')
        try {
          const r = await apiPost('/api/settings/launcher', { launcher: 'custom' })
          setLauncherUI(r.launcher)
          toast('Лаунчер: свой')
        } catch (err) {
          setLauncherUI(prev)
          toast(err.message, true)
        }
      } else {
        pickCustomLauncher()
      }
      return
    }
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

document.getElementById('stats-btn').addEventListener('click', openStats)

document.getElementById('stats-close').addEventListener('click', closeStats)

document.getElementById('logs-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'logs-overlay') closeLogs()
})

document.getElementById('stats-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'stats-overlay') closeStats()
})

document.getElementById('logs-clear').addEventListener('click', async () => {
  const info = await apiPost('/api/logs/clear?info=1', {})
  const mb = ((info.bytes || 0) / 1048576).toFixed(2)
  if (!(await confirmDialog('Удалить все логи?', 'Вы действительно хотите удалить все записанные логи всех аккаунтов на ' + mb + ' MB?'))) return
  await apiPost('/api/logs/clear', {})
  resetLogsSelection()
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

loadCaps()

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
    let text
    if (el.hasAttribute('title')) {
      text = el.getAttribute('title')
      el.setAttribute('data-tip', text)
      el.removeAttribute('title')
    } else {
      text = el.getAttribute('data-tip')
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
