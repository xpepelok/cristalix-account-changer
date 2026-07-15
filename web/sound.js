const Sound = (() => {
  let ctx = null
  let master = null
  let last = 0
  let enabled = localStorage.getItem('ac-sound') !== '0'

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
    if (ctx.state === 'suspended') ctx.resume()
    return ctx
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
      const t = ctx.currentTime
      blip(440, 'sine', t, 0.05, 0.04, { attack: 0.007, glide: 380 })
    },
    confirm() {
      const t = ctx.currentTime
      blip(523.25, 'sine', t, 0.13, 0.1)
      blip(783.99, 'sine', t + 0.045, 0.17, 0.08, { detune: 1 })
    },
    gear() {
      const t = ctx.currentTime
      blip(392, 'sine', t, 0.05, 0.038, { attack: 0.007, glide: 340 })
      blip(588, 'sine', t + 0.045, 0.07, 0.03, { attack: 0.007 })
    },
    bell() {
      const t = ctx.currentTime
      const base = 784
      const partials = [[1, 0.11], [2.01, 0.055], [3.02, 0.032], [4.24, 0.02]]
      partials.forEach((p) => blip(base * p[0], 'sine', t, 0.55 + (1 / p[0]) * 0.35, p[1], { attack: 0.004 }))
    },
    theme(toLight) {
      const t = ctx.currentTime
      if (toLight) {
        blip(587.33, 'sine', t, 0.14, 0.09)
        blip(880, 'sine', t + 0.055, 0.22, 0.08, { detune: 2 })
      } else {
        blip(880, 'sine', t, 0.14, 0.09)
        blip(587.33, 'sine', t + 0.055, 0.22, 0.08)
      }
    },
  }

  function play(name, arg) {
    if (!enabled) return
    if (!ensure()) return
    const now = ctx.currentTime
    if (name === 'tap' && now - last < 0.04) return
    last = now
    const fn = sounds[name]
    if (fn) {
      try {
        fn(arg)
      } catch (e) {
        /* ignore */
      }
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
