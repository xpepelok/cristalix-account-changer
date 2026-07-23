function renderLaunchQueue(list) {
  const dd = document.getElementById('queue-dd')
  if (!dd) return
  const ring = document.getElementById('queue-ring')
  const count = document.getElementById('queue-count')
  const menu = document.getElementById('queue-menu')
  const n = list.length

  if (n === 0) {
    if (!dd.hidden) closeQueueMenu()
    dd.hidden = true
    state.queuePeak = 0
    return
  }

  dd.hidden = false
  if (n > (state.queuePeak || 0)) state.queuePeak = n
  const peak = state.queuePeak || n
  const fill = Math.max(0, Math.min(100, Math.round(((peak - n) / peak) * 100)))
  ring.style.setProperty('--fill', fill)
  count.textContent = n

  menu.innerHTML =
    '<div class="queue-menu-title">Очередь запуска</div>' +
    list
      .map((e) => {
        const acc = state.byUuid.get(e.uuid)
        const g = acc ? accountGroupLabel(acc) : null
        if (e.name) ensurePlayerInfo(e.name)
        const chip = g
          ? '<span class="logs-chip" style="--chip:' + g.color + ';color:' + g.color + '">' + esc(g.label) + '</span>'
          : ''
        return (
          '<div class="queue-item">' +
          '<span class="queue-item-ava head-ava" style="--skin:' +
          skinBg(e.uuid) +
          '"></span>' +
          chip +
          '<span class="queue-item-name">' +
          esc(e.name || shortId(e.uuid)) +
          '</span><span class="queue-item-state' +
          (e.state === 'launching' ? ' on' : '') +
          '">' +
          (e.state === 'launching' ? 'запуск' : 'в очереди') +
          '</span></div>'
        )
      })
      .join('')
}

function positionQueueMenu() {
  const menu = document.getElementById('queue-menu')
  const btn = document.getElementById('queue-btn')
  if (!menu || !btn) return
  const r = btn.getBoundingClientRect()
  menu.style.left = 'auto'
  menu.style.top = 'auto'
  menu.style.right = window.innerWidth - r.right + 'px'
  menu.style.bottom = window.innerHeight - r.top + 8 + 'px'
}

function openQueueMenu() {
  const dd = document.getElementById('queue-dd')
  const menu = document.getElementById('queue-menu')
  if (!dd || !menu) return
  dd.classList.add('open')
  showMenu(menu)
  positionQueueMenu()
}

function closeQueueMenu() {
  const dd = document.getElementById('queue-dd')
  if (!dd) return
  dd.classList.remove('open')
  hideMenuAnimated(document.getElementById('queue-menu'))
}

async function pollLaunchQueue() {
  try {
    const list = await apiGet('/api/launch-queue')
    state.launchQueue = Array.isArray(list) ? list : []
    renderLaunchQueue(state.launchQueue)
  } catch (e) {
    /* ignore */
  }
}

;(function initLaunchQueue() {
  const btn = document.getElementById('queue-btn')
  if (!btn) return
  btn.addEventListener('click', (e) => {
    e.stopPropagation()
    const dd = document.getElementById('queue-dd')
    if (dd.classList.contains('open')) closeQueueMenu()
    else openQueueMenu()
  })
  document.addEventListener('click', (e) => {
    if (!e.target.closest('#queue-dd')) closeQueueMenu()
  })
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeQueueMenu()
  })
  window.addEventListener('resize', () => {
    if (document.getElementById('queue-dd').classList.contains('open')) positionQueueMenu()
  })
  pollLaunchQueue()
  setInterval(pollLaunchQueue, 700)
})()
