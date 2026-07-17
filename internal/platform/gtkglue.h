//go:build linux && cgo

#ifndef AC_GTK_GLUE_H
#define AC_GTK_GLUE_H

int ac_window_init(const char *url, const char *data_path, const char *bridge_js,
                   const unsigned char *icon_data, int icon_len);
void ac_window_main(void);
void ac_window_present(void);
void ac_window_quit(void);
void ac_window_iconify(void);
void ac_window_maximize_toggle(void);
void ac_window_drag(void);
void ac_window_resize(int edge);
int ac_clipboard_set(const char *text);
char *ac_pick_executable(void);
void ac_eval_js(const char *script);
void ac_free(void *p);

#endif
