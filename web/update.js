function updateFilterBadge() {
  const badge = document.getElementById('filter-badge')
  const n = activeFilterCount()
  badge.hidden = n === 0
  badge.textContent = n
}

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

function updateCustomPathUI() {
  const el = document.getElementById('custom-launcher-path')
  if (customLauncherPath) {
    el.textContent = baseName(customLauncherPath)
    el.title = customLauncherPath
  } else {
    el.textContent = 'Файл не выбран'
    el.title = ''
  }
}
