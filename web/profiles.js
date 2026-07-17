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
