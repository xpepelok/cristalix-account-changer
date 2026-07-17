//go:build linux && cgo

package platform

/*
#cgo pkg-config: gtk+-3.0
#cgo !webkit40 pkg-config: webkit2gtk-4.1
#cgo webkit40 pkg-config: webkit2gtk-4.0

#include <stdlib.h>
#include "gtkglue.h"
*/
import "C"

import (
	"encoding/json"
	"net/url"
	"strconv"
	"strings"
	"unsafe"
)

const bridgeJS = `
(function () {
  if (window.__acBridge) return;
  var pending = new Map();
  var seq = 0;
  window.__acResolve = function (id, ok, value) {
    var p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    if (ok) p.resolve(value); else p.reject(new Error(String(value)));
  };
  function call(method, args) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      pending.set(id, { resolve: resolve, reject: reject });
      try {
        window.webkit.messageHandlers.ac.postMessage(
          JSON.stringify({ id: id, method: method, args: args }));
      } catch (e) {
        pending.delete(id);
        reject(e);
      }
    });
  }
  ['acQuit', 'acMinimize', 'acHide', 'acDrag', 'acResize', 'acMaximize',
   'acOpenUrl', 'acCopy', 'acPickLauncher'].forEach(function (m) {
    window[m] = function () { return call(m, Array.prototype.slice.call(arguments)); };
  });
  window.__acBridge = true;
})();
`

var resizeEdges = map[int]int{
	10: 3,
	11: 4,
	12: 1,
	13: 0,
	14: 2,
	15: 6,
	16: 5,
	17: 7,
}

func PickExecutable() string {
	cpath := C.ac_pick_executable()
	if cpath == nil {
		return ""
	}
	defer C.ac_free(unsafe.Pointer(cpath))
	return C.GoString(cpath)
}

func RunNativeWindow(appURL, dataPath string, iconPNG []byte, onReady func(focus, quit func())) bool {
	curl := C.CString(appURL)
	defer C.free(unsafe.Pointer(curl))
	cpath := C.CString(dataPath)
	defer C.free(unsafe.Pointer(cpath))
	cjs := C.CString(bridgeJS)
	defer C.free(unsafe.Pointer(cjs))

	var iconPtr *C.uchar
	if len(iconPNG) > 0 {
		iconPtr = (*C.uchar)(unsafe.Pointer(&iconPNG[0]))
	}

	if C.ac_window_init(curl, cpath, cjs, iconPtr, C.int(len(iconPNG))) == 0 {
		return false
	}
	if onReady != nil {
		onReady(
			func() { C.ac_window_present() },
			func() { C.ac_window_quit() },
		)
	}
	C.ac_window_main()
	return true
}

type bridgeCall struct {
	ID     int64             `json:"id"`
	Method string            `json:"method"`
	Args   []json.RawMessage `json:"args"`
}

//export acHandleMessage
func acHandleMessage(raw *C.char) {
	var call bridgeCall
	if json.Unmarshal([]byte(C.GoString(raw)), &call) != nil {
		return
	}
	result, err := dispatchBridge(call)
	resolveBridge(call.ID, result, err)
}

func dispatchBridge(call bridgeCall) (any, error) {
	switch call.Method {
	case "acQuit":
		C.ac_window_quit()
	case "acMinimize":
		C.ac_window_iconify()
	case "acHide":
		C.ac_window_iconify()
		TrimMemory()
	case "acDrag":
		C.ac_window_drag()
	case "acMaximize":
		C.ac_window_maximize_toggle()
	case "acResize":
		code, ok := argInt(call.Args, 0)
		if !ok {
			return nil, nil
		}
		if edge, known := resizeEdges[code]; known {
			C.ac_window_resize(C.int(edge))
		}
	case "acOpenUrl":
		if link, ok := argString(call.Args, 0); ok && externalLinkAllowed(link) {
			openExternal(link)
		}
	case "acCopy":
		text, ok := argString(call.Args, 0)
		if !ok {
			return false, nil
		}
		ctext := C.CString(text)
		defer C.free(unsafe.Pointer(ctext))
		return C.ac_clipboard_set(ctext) != 0, nil
	case "acPickLauncher":
		return PickExecutable(), nil
	}
	return nil, nil
}

func externalLinkAllowed(link string) bool {
	u, err := url.Parse(link)
	if err != nil {
		return false
	}
	return u.Scheme == "http" || u.Scheme == "https"
}

func argString(args []json.RawMessage, i int) (string, bool) {
	if i >= len(args) {
		return "", false
	}
	var s string
	if json.Unmarshal(args[i], &s) != nil {
		return "", false
	}
	return s, true
}

func argInt(args []json.RawMessage, i int) (int, bool) {
	if i >= len(args) {
		return 0, false
	}
	var f float64
	if json.Unmarshal(args[i], &f) != nil {
		return 0, false
	}
	return int(f), true
}

func resolveBridge(id int64, result any, err error) {
	ok := "true"
	payload := result
	if err != nil {
		ok = "false"
		payload = err.Error()
	}
	encoded, jerr := json.Marshal(payload)
	if jerr != nil {
		encoded = []byte("null")
	}

	var js strings.Builder
	js.WriteString("window.__acResolve(")
	js.WriteString(strconv.FormatInt(id, 10))
	js.WriteString(",")
	js.WriteString(ok)
	js.WriteString(",")
	js.Write(encoded)
	js.WriteString(");")

	cjs := C.CString(js.String())
	defer C.free(unsafe.Pointer(cjs))
	C.ac_eval_js(cjs)
}
