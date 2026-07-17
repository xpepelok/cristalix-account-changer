package main

import (
	"io/fs"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestEmbeddedWebFilesAreServed(t *testing.T) {
	s := &Server{}
	h := s.handler()

	sub, err := fs.Sub(webFiles, "web")
	if err != nil {
		t.Fatal(err)
	}
	entries, err := fs.ReadDir(sub, ".")
	if err != nil {
		t.Fatal(err)
	}

	var served int
	for _, e := range entries {
		if e.IsDir() || e.Name() == "index.html" {
			continue
		}
		rec := httptest.NewRecorder()
		h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/"+e.Name(), nil))
		if rec.Code != http.StatusOK {
			t.Errorf("%s: код %d, ожидался 200", e.Name(), rec.Code)
			continue
		}
		if rec.Body.Len() == 0 {
			t.Errorf("%s: пустое тело", e.Name())
		}
		served++
	}
	if served == 0 {
		t.Fatal("ни один файл не раздан")
	}
	t.Logf("раздано файлов: %d", served)
}

func TestIndexReferencesExistOnDisk(t *testing.T) {
	sub, err := fs.Sub(webFiles, "web")
	if err != nil {
		t.Fatal(err)
	}
	index, err := fs.ReadFile(sub, "index.html")
	if err != nil {
		t.Fatal(err)
	}

	s := &Server{}
	h := s.handler()

	var checked int
	for _, part := range strings.Split(string(index), `src="`)[1:] {
		src := part[:strings.Index(part, `"`)]
		if !strings.HasPrefix(src, "/") {
			continue
		}
		rec := httptest.NewRecorder()
		h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, src, nil))
		if rec.Code != http.StatusOK {
			t.Errorf("index.html ссылается на %s, но сервер отдаёт %d", src, rec.Code)
		}
		checked++
	}
	if checked == 0 {
		t.Fatal("в index.html не найдено ни одной ссылки на скрипт")
	}
	t.Logf("проверено ссылок из index.html: %d", checked)
}

func TestNoStaleAppJS(t *testing.T) {
	sub, err := fs.Sub(webFiles, "web")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := fs.Stat(sub, "app.js"); err == nil {
		t.Error("web/app.js всё ещё встроен в бинарник")
	}
}
