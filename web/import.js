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
