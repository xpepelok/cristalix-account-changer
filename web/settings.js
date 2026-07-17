async function togglePin(acc) {
  if (!acc) return
  try {
    await apiPost('/api/pin', { uuid: acc.uuid, pinned: !acc.pinned })
    loadAccounts()
  } catch (e) {
    toast(e.message, true)
  }
}

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

function setToggle(id, on) {
  const t = document.getElementById(id)
  t.classList.toggle('on', on)
  t.setAttribute('aria-checked', on ? 'true' : 'false')
}

function toggleLauncherMenu(force) {
  const dd = document.getElementById('launcher-dd')
  const menu = document.getElementById('launcher-dd-menu')
  const open = force !== undefined ? force : menu.hidden
  menu.hidden = !open
  dd.classList.toggle('open', open)
}

async function loadCaps() {
  try {
    caps = await apiGet('/api/caps')
  } catch (e) {
    return
  }
  applyCaps()
}

function applyCaps() {
  if (!caps.credentialImport) {
    hideEl('cred-btn')
    hideEl('refresh-tokens-btn')
  }
  if (!caps.autoPlay) {
    hideRow('toggle-autoplay')
  }
  if (!caps.exeLaunchers) {
    document.querySelectorAll('#launcher-dd-menu .launcher-dd-item').forEach((it) => {
      if (it.dataset.val === 'normal' || it.dataset.val === 'new') it.hidden = true
    })
  }
  if (!caps.tray) {
    setText('win-close', 'title', 'Свернуть — приложение продолжит ловить токены в фоне')
  }
  if (caps.os && caps.os !== 'windows') {
    const desc = document.querySelector('#toggle-autostart')?.closest('.setting-row')?.querySelector('.setting-desc')
    if (desc) desc.textContent = 'Запускать AccountChanger при входе в систему'
  }
}

function hideEl(id) {
  const el = document.getElementById(id)
  if (el) el.hidden = true
}

function hideRow(id) {
  const row = document.getElementById(id)?.closest('.setting-row')
  if (row) row.hidden = true
}

function setText(id, attr, value) {
  const el = document.getElementById(id)
  if (!el) return
  el.setAttribute(attr, value)
  if (attr === 'title') el.setAttribute('aria-label', value)
}

async function openSettings() {
  document.getElementById('settings-overlay').hidden = false
  setToggle('toggle-sound', window.soundEnabled ? window.soundEnabled() : true)
  try {
    const s = await apiGet('/api/settings')
    setToggle('toggle-autostart', !!s.autostart)
    setToggle('toggle-autoplay', s.autoPlay !== false)
    setToggle('toggle-stats', s.stats !== false)
    customLauncherPath = s.customLauncher || ''
    updateCustomPathUI()
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
