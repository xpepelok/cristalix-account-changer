function skinBg(uuid) {
  return `url('/skin/${uuid}')`
}

function applyPlayerInfo(card, acc) {
  const info = acc.name ? state.playerInfo.get(acc.name.toLowerCase()) : null
  const grp = primaryGroup(info)
  if (grp) {
    card._chip.hidden = false
    card._chip.textContent = grp.label
    card._chip.style.setProperty('--chip', grp.color)
    card.style.setProperty('--grp', grp.color)
  } else {
    card._chip.hidden = true
    card.style.removeProperty('--grp')
  }
  const online = info ? info.online : ''
  card._dot.className = 'online-dot' + (online === 'online' ? ' on' : online === 'offline' ? ' off' : '')
  card._dot.title = online === 'online' ? 'В сети' : online === 'offline' ? 'Не в сети' : ''
}

async function ensurePlayerInfo(name) {
  const key = name.toLowerCase()
  if (state.playerInfo.has(key) || state.pendingInfo.has(key)) return
  state.pendingInfo.add(key)
  try {
    const info = await apiGet('/api/player?name=' + encodeURIComponent(name))
    if (info && info.name) state.playerInfo.set(key, info)
    else if (!state.playerInfo.get(key)) state.playerInfo.set(key, null)
  } catch (e) {
    if (!state.playerInfo.get(key)) state.playerInfo.set(key, null)
  }
  state.pendingInfo.delete(key)
  state.cards.forEach((card) => {
    const acc = cardAccount(card)
    if (acc && acc.name && acc.name.toLowerCase() === key) applyPlayerInfo(card, acc)
  })
  if (state.selected && state.selected.name && state.selected.name.toLowerCase() === key) {
    renderModalGroups(state.selected)
    renderModalStats(state.selected)
  }
}

function subLabel(key) {
  const m = /^level_(\d+)$/.exec(key || '')
  if (m) return 'Ур. ' + m[1]
  return key
}

function loadSkinLib() {
  if (window.skinview3d) return Promise.resolve()
  if (!skinLibPromise) {
    skinLibPromise = new Promise((resolve, reject) => {
      const s = document.createElement('script')
      s.src = '/skinview3d.bundle.js'
      s.onload = resolve
      s.onerror = reject
      document.head.appendChild(s)
    })
  }
  return skinLibPromise
}

async function textureExists(kind, uuid) {
  try {
    const r = await fetch(`/${kind}/${uuid}`, { method: 'HEAD' })
    return r.ok
  } catch (e) {
    return false
  }
}

function getTextureAvailability(uuid) {
  if (textureCache.has(uuid)) return textureCache.get(uuid)
  const p = Promise.all([textureExists('skin', uuid), textureExists('cape', uuid)]).then(([hasSkin, hasCape]) => ({
    hasSkin,
    hasCape,
  }))
  textureCache.set(uuid, p)
  return p
}

function downloadTexture(kind, uuid, name) {
  const a = document.createElement('a')
  a.href = `/${kind}/${uuid}`
  a.download = (name || uuid) + '_' + kind + '.png'
  document.body.appendChild(a)
  a.click()
  a.remove()
}

async function mountSkin(uuid) {
  disposeSkin()
  state.skinToken = uuid
  const dlWrap = document.getElementById('skin-downloads')
  const dlSkinBtn = document.getElementById('dl-skin-btn')
  const dlCapeBtn = document.getElementById('dl-cape-btn')
  dlWrap.hidden = true
  dlSkinBtn.hidden = true
  dlCapeBtn.hidden = true

  let avail
  try {
    const [, a] = await Promise.all([loadSkinLib(), getTextureAvailability(uuid)])
    avail = a
  } catch (e) {
    return
  }
  if (state.skinToken !== uuid) return

  if (avail.hasSkin || avail.hasCape) {
    dlWrap.hidden = false
    if (avail.hasSkin) {
      dlSkinBtn.hidden = false
      dlSkinBtn.onclick = () => downloadTexture('skin', uuid, state.selected && state.selected.name)
    }
    if (avail.hasCape) {
      dlCapeBtn.hidden = false
      dlCapeBtn.onclick = () => downloadTexture('cape', uuid, state.selected && state.selected.name)
    }
  }

  const canvas = document.createElement('canvas')
  canvas.className = 'skin-canvas'
  skinStage.appendChild(canvas)

  const viewer = new skinview3d.SkinViewer({
    canvas,
    width: skinStage.clientWidth || 210,
    height: skinStage.clientHeight || 320,
  })
  state.viewer = viewer

  viewer.animation = new skinview3d.IdleAnimation()
  viewer.autoRotate = false
  viewer.controls.enableZoom = true
  viewer.controls.enablePan = false
  viewer.playerObject.rotation.y = 0.5
  viewer.zoom = DEFAULT_ZOOM

  const measure = () => ({
    width: skinStage.clientWidth || 210,
    height: skinStage.clientHeight || 320,
  })
  let lastW = 0
  let lastH = 0
  const ro = new ResizeObserver(() => {
    if (!state.viewer) return
    const { width, height } = measure()
    if (width <= 0 || height <= 0) return
    if (Math.abs(width - lastW) < 1 && Math.abs(height - lastH) < 1) return
    lastW = width
    lastH = height
    viewer.setSize(width, height)
  })
  ro.observe(skinStage)
  viewer._ro = ro

  viewer.loadSkin(`/skin/${uuid}`, { model: 'auto-detect' }).catch(() => {})
  viewer.loadCape(`/cape/${uuid}`).catch(() => {})
}

function disposeSkin() {
  state.skinToken = null
  if (state.viewer) {
    if (state.viewer._ro) state.viewer._ro.disconnect()
    state.viewer.dispose()
    state.viewer = null
  }
  skinStage.innerHTML = ''
}

async function refreshPlayerInfo() {
  const names = new Set()
  state.accounts.forEach((a) => {
    if (a.name) names.add(a.name)
  })
  for (const name of names) {
    const key = name.toLowerCase()
    try {
      const info = await apiGet('/api/player?name=' + encodeURIComponent(name))
      if (info && info.name) state.playerInfo.set(key, info)
      else if (!state.playerInfo.get(key)) state.playerInfo.set(key, null)
    } catch (e) {
      /* keep previous */
    }
  }
  state.cards.forEach((card) => {
    const acc = cardAccount(card)
    if (acc) applyPlayerInfo(card, acc)
  })
  if (state.selected) {
    renderModalGroups(state.selected)
    renderModalStats(state.selected)
  }
}
