//go:build linux && cgo

#include <gtk/gtk.h>
#include <webkit2/webkit2.h>
#include <stdlib.h>

#include "gtkglue.h"
#include "_cgo_export.h"

static GtkWidget *ac_window = NULL;
static GtkWidget *ac_webview = NULL;

static void ac_script_message(WebKitUserContentManager *mgr, WebKitJavascriptResult *res, gpointer data) {
  (void)mgr;
  (void)data;
  JSCValue *value = webkit_javascript_result_get_js_value(res);
  char *json = jsc_value_to_string(value);
  if (json) {
    acHandleMessage(json);
    g_free(json);
  }
}

static gboolean ac_delete_event(GtkWidget *w, GdkEvent *e, gpointer d) {
  (void)w;
  (void)e;
  (void)d;
  gtk_main_quit();
  return TRUE;
}

static GdkPixbuf *ac_pixbuf_from_png(const unsigned char *data, int len) {
  if (!data || len <= 0) {
    return NULL;
  }
  GInputStream *stream = g_memory_input_stream_new_from_data(data, (gssize)len, NULL);
  GdkPixbuf *pixbuf = gdk_pixbuf_new_from_stream(stream, NULL, NULL);
  g_object_unref(stream);
  return pixbuf;
}

int ac_window_init(const char *url, const char *data_path, const char *bridge_js,
                   const unsigned char *icon_data, int icon_len) {
  g_set_prgname("accountchanger");
  if (!gtk_init_check(NULL, NULL)) {
    return 0;
  }
  gdk_set_program_class("Accountchanger");

  WebKitWebsiteDataManager *dm = webkit_website_data_manager_new(
      "base-data-directory", data_path,
      "base-cache-directory", data_path,
      NULL);
  if (!dm) {
    return 0;
  }
  WebKitWebContext *ctx = webkit_web_context_new_with_website_data_manager(dm);
  if (!ctx) {
    return 0;
  }

  WebKitUserContentManager *ucm = webkit_user_content_manager_new();
  webkit_user_content_manager_register_script_message_handler(ucm, "ac");
  g_signal_connect(ucm, "script-message-received::ac", G_CALLBACK(ac_script_message), NULL);

  WebKitUserScript *script = webkit_user_script_new(
      bridge_js,
      WEBKIT_USER_CONTENT_INJECT_TOP_FRAME,
      WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START,
      NULL, NULL);
  webkit_user_content_manager_add_script(ucm, script);
  webkit_user_script_unref(script);

  ac_webview = GTK_WIDGET(g_object_new(WEBKIT_TYPE_WEB_VIEW,
                                       "web-context", ctx,
                                       "user-content-manager", ucm,
                                       NULL));
  if (!ac_webview) {
    return 0;
  }

  ac_window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(ac_window), "AccountChanger");
  gtk_window_set_default_size(GTK_WINDOW(ac_window), 1180, 820);
  gtk_widget_set_size_request(ac_window, 810, 600);
  gtk_window_set_position(GTK_WINDOW(ac_window), GTK_WIN_POS_CENTER);
  gtk_window_set_decorated(GTK_WINDOW(ac_window), FALSE);
  gtk_container_add(GTK_CONTAINER(ac_window), ac_webview);
  g_signal_connect(ac_window, "delete-event", G_CALLBACK(ac_delete_event), NULL);

  GdkPixbuf *icon = ac_pixbuf_from_png(icon_data, icon_len);
  if (icon) {
    gtk_window_set_default_icon(icon);
    gtk_window_set_icon(GTK_WINDOW(ac_window), icon);
    g_object_unref(icon);
  }

  webkit_web_view_load_uri(WEBKIT_WEB_VIEW(ac_webview), url);
  gtk_widget_show_all(ac_window);
  return 1;
}

void ac_window_main(void) {
  gtk_main();
}

static gboolean ac_present_cb(gpointer d) {
  (void)d;
  if (ac_window) {
    gtk_window_deiconify(GTK_WINDOW(ac_window));
    gtk_window_present(GTK_WINDOW(ac_window));
  }
  return G_SOURCE_REMOVE;
}

void ac_window_present(void) {
  g_idle_add(ac_present_cb, NULL);
}

static gboolean ac_quit_cb(gpointer d) {
  (void)d;
  gtk_main_quit();
  return G_SOURCE_REMOVE;
}

void ac_window_quit(void) {
  g_idle_add(ac_quit_cb, NULL);
}

void ac_window_iconify(void) {
  if (ac_window) {
    gtk_window_iconify(GTK_WINDOW(ac_window));
  }
}

void ac_window_maximize_toggle(void) {
  if (!ac_window) {
    return;
  }
  if (gtk_window_is_maximized(GTK_WINDOW(ac_window))) {
    gtk_window_unmaximize(GTK_WINDOW(ac_window));
  } else {
    gtk_window_maximize(GTK_WINDOW(ac_window));
  }
}

static gboolean ac_pointer_pos(int *x, int *y) {
  if (!ac_window) {
    return FALSE;
  }
  GdkDisplay *display = gtk_widget_get_display(ac_window);
  if (!display) {
    return FALSE;
  }
  GdkSeat *seat = gdk_display_get_default_seat(display);
  if (!seat) {
    return FALSE;
  }
  GdkDevice *pointer = gdk_seat_get_pointer(seat);
  if (!pointer) {
    return FALSE;
  }
  gdk_device_get_position(pointer, NULL, x, y);
  return TRUE;
}

void ac_window_drag(void) {
  int x = 0, y = 0;
  if (!ac_pointer_pos(&x, &y)) {
    return;
  }
  gtk_window_begin_move_drag(GTK_WINDOW(ac_window), 1, x, y, GDK_CURRENT_TIME);
}

void ac_window_resize(int edge) {
  if (edge < 0) {
    return;
  }
  int x = 0, y = 0;
  if (!ac_pointer_pos(&x, &y)) {
    return;
  }
  if (gtk_window_is_maximized(GTK_WINDOW(ac_window))) {
    return;
  }
  gtk_window_begin_resize_drag(GTK_WINDOW(ac_window), (GdkWindowEdge)edge, 1, x, y, GDK_CURRENT_TIME);
}

int ac_clipboard_set(const char *text) {
  GtkClipboard *cb = gtk_clipboard_get(GDK_SELECTION_CLIPBOARD);
  if (!cb) {
    return 0;
  }
  gtk_clipboard_set_text(cb, text, -1);
  gtk_clipboard_store(cb);
  return 1;
}

char *ac_pick_executable(void) {
  GtkWidget *dlg = gtk_file_chooser_dialog_new(
      "Выберите файл лаунчера",
      ac_window ? GTK_WINDOW(ac_window) : NULL,
      GTK_FILE_CHOOSER_ACTION_OPEN,
      "_Отмена", GTK_RESPONSE_CANCEL,
      "_Открыть", GTK_RESPONSE_ACCEPT,
      NULL);
  if (!dlg) {
    return NULL;
  }

  GtkFileFilter *launchers = gtk_file_filter_new();
  gtk_file_filter_set_name(launchers, "Лаунчер (*.jar, *.sh, *.AppImage)");
  gtk_file_filter_add_pattern(launchers, "*.jar");
  gtk_file_filter_add_pattern(launchers, "*.sh");
  gtk_file_filter_add_pattern(launchers, "*.AppImage");
  gtk_file_chooser_add_filter(GTK_FILE_CHOOSER(dlg), launchers);

  GtkFileFilter *all = gtk_file_filter_new();
  gtk_file_filter_set_name(all, "Все файлы");
  gtk_file_filter_add_pattern(all, "*");
  gtk_file_chooser_add_filter(GTK_FILE_CHOOSER(dlg), all);

  char *out = NULL;
  if (gtk_dialog_run(GTK_DIALOG(dlg)) == GTK_RESPONSE_ACCEPT) {
    out = gtk_file_chooser_get_filename(GTK_FILE_CHOOSER(dlg));
  }
  gtk_widget_destroy(dlg);
  return out;
}

void ac_eval_js(const char *script) {
  if (!ac_webview) {
    return;
  }
  webkit_web_view_evaluate_javascript(WEBKIT_WEB_VIEW(ac_webview), script, -1,
                                      NULL, NULL, NULL, NULL, NULL);
}

void ac_free(void *p) {
  g_free(p);
}
