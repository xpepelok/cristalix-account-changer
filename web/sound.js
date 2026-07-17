const Sound = (() => {
  let ctx = null
  let master = null
  let last = 0
  let enabled = localStorage.getItem('ac-sound') !== '0'

  const LOOKAHEAD = 0.02

  function ensure() {
    if (!ctx) {
      const AC = window.AudioContext || window.webkitAudioContext
      if (!AC) return null
      ctx = new AC()
      master = ctx.createGain()
      master.gain.value = 0.42
      const lp = ctx.createBiquadFilter()
      lp.type = 'lowpass'
      lp.frequency.value = 7600
      master.connect(lp)
      lp.connect(ctx.destination)
    }
    return ctx
  }

  function at() {
    return ctx.currentTime + LOOKAHEAD
  }

  function blip(freq, type, start, dur, peak, opts) {
    opts = opts || {}
    const o = ctx.createOscillator()
    const g = ctx.createGain()
    o.type = type
    o.frequency.setValueAtTime(freq, start)
    if (opts.glide) o.frequency.exponentialRampToValueAtTime(opts.glide, start + dur * 0.85)
    if (opts.detune) o.detune.value = opts.detune
    o.connect(g)
    g.connect(master)
    g.gain.setValueAtTime(0.0001, start)
    g.gain.exponentialRampToValueAtTime(peak, start + (opts.attack || 0.006))
    g.gain.exponentialRampToValueAtTime(0.0001, start + dur)
    o.start(start)
    o.stop(start + dur + 0.04)
  }

  const sounds = {
    tap() {
      const t = at()
      blip(440, 'sine', t, 0.05, 0.04, { attack: 0.007, glide: 380 })
    },
    confirm() {
      const t = at()
      blip(523.25, 'sine', t, 0.13, 0.1)
      blip(783.99, 'sine', t + 0.045, 0.17, 0.08, { detune: 1 })
    },
    gear() {
      const t = at()
      blip(392, 'sine', t, 0.05, 0.038, { attack: 0.007, glide: 340 })
      blip(588, 'sine', t + 0.045, 0.07, 0.03, { attack: 0.007 })
    },
    bell() {
      const t = at()
      const base = 784
      const partials = [[1, 0.11], [2.01, 0.055], [3.02, 0.032], [4.24, 0.02]]
      partials.forEach((p) => blip(base * p[0], 'sine', t, 0.55 + (1 / p[0]) * 0.35, p[1], { attack: 0.004 }))
    },
    stats() {
      const t = at()
      blip(440, 'sine', t, 0.06, 0.032, { attack: 0.005 })
      blip(554.37, 'sine', t + 0.04, 0.07, 0.035, { attack: 0.005 })
      blip(659.25, 'sine', t + 0.08, 0.1, 0.04, { attack: 0.005 })
    },
    logs() {
      const t = at()
      blip(494, 'sine', t, 0.06, 0.036, { attack: 0.005, glide: 420 })
      blip(740, 'sine', t + 0.05, 0.09, 0.03, { attack: 0.006 })
    },
    profiles() {
      const t = at()
      blip(494, 'sine', t, 0.16, 0.06, { attack: 0.006 })
      blip(622.25, 'sine', t + 0.02, 0.18, 0.05, { attack: 0.006, detune: 3 })
    },
    theme(toLight) {
      const t = at()
      if (toLight) {
        blip(587.33, 'sine', t, 0.14, 0.09)
        blip(880, 'sine', t + 0.055, 0.22, 0.08, { detune: 2 })
      } else {
        blip(880, 'sine', t, 0.14, 0.09)
        blip(587.33, 'sine', t + 0.055, 0.22, 0.08)
      }
    },
  }

  function fire(fn, arg) {
    try {
      fn(arg)
    } catch (e) {
      /* ignore */
    }
  }

  function play(name, arg) {
    if (!enabled) return
    if (!ensure()) return
    const fn = sounds[name]
    if (!fn) return
    if (name === 'tap') {
      const now = performance.now()
      if (now - last < 40) return
      last = now
    }
    if (ctx.state === 'running') {
      fire(fn, arg)
      return
    }
    const resumed = ctx.resume()
    if (resumed && typeof resumed.then === 'function') {
      resumed.then(() => fire(fn, arg)).catch(() => {})
    } else {
      fire(fn, arg)
    }
  }

  function setEnabled(v) {
    enabled = !!v
    localStorage.setItem('ac-sound', v ? '1' : '0')
  }

  return { play, setEnabled, isEnabled: () => enabled }
})()

window.playSound = Sound.play
window.setSoundEnabled = Sound.setEnabled
window.soundEnabled = Sound.isEnabled
