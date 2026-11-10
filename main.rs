#![windows_subsystem = "windows"]

mod sys;

use eframe::egui;
use egui::{pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Stroke};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(0x14, 0x10, 0x2b);
const PANEL: Color32 = Color32::from_rgb(0x22, 0x1a, 0x47);
// полупрозрачная панель, чтобы фон просвечивал
const PANEL_A: Color32 = Color32::from_rgba_premultiplied(0x1e, 0x17, 0x3f, 225);
const FG: Color32 = Color32::from_rgb(0xf0, 0xee, 0xff);
const MUTED: Color32 = Color32::from_rgb(0xa7, 0x9f, 0xd6);
const ACCENT: Color32 = Color32::from_rgb(0x7b, 0x5c, 0xff);
const ACCENT_H: Color32 = Color32::from_rgb(0x95, 0x7c, 0xff);
const AMBER: Color32 = Color32::from_rgb(0xe0, 0xa0, 0x30);
const GREEN: Color32 = Color32::from_rgb(0x4c, 0xd9, 0x8a);
const RED: Color32 = Color32::from_rgb(0xe8, 0x5a, 0x6a);

const WIN_W: f32 = 980.0;
const WIN_H: f32 = 620.0;
const LEFT_W: f32 = 380.0;

fn muted(s: impl Into<String>) -> RichText {
    RichText::new(s).color(MUTED)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

fn card(ui: &mut egui::Ui, w: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(PANEL_A)
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(w - 24.0);
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            add(ui);
        });
}

fn bar(ui: &mut egui::Ui, w: f32, frac: f32, col: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 5.0, BG);
    let fill = Rect::from_min_size(rect.min, vec2((w * frac.clamp(0.0, 1.0)).max(6.0), 12.0));
    ui.painter().rect_filled(fill, 5.0, col);
}

/// Полоска «таблетки» с горизонтальным градиентом и закруглёнными краями.
fn pill(p: &egui::Painter, x1: f32, y1: f32, x2: f32, y2: f32, c0: Color32, c1: Color32) {
    let r = 14.0_f32;
    let span = (x2 - x1).max(1.0);
    let mut x = x1 + 2.0;
    while x < x2 - 1.0 {
        let dx = (x - x1).min(x2 - x);
        let inset = if dx >= r { 0.0 } else { r - (r * r - (r - dx) * (r - dx)).max(0.0).sqrt() };
        p.line_segment(
            [pos2(x, y1 + inset), pos2(x, y2 - inset)],
            Stroke::new(4.0, mix(c0, c1, (x - x1) / span)),
        );
        x += 3.0;
    }
}

fn big_button(ui: &mut egui::Ui, w: f32, label: &str, sub: &str, enabled: bool, bold: &egui::FontFamily) -> bool {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 66.0), sense);
    let hover = enabled && resp.hovered();
    let pressed = enabled && resp.is_pointer_button_down_on();
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let off = if pressed { 2.0 } else { 0.0 };
    let (c0, c1, sh) = if !enabled {
        (
            Color32::from_rgb(0x3a, 0x34, 0x66),
            Color32::from_rgb(0x46, 0x3d, 0x7a),
            Color32::from_rgb(0x26, 0x20, 0x48),
        )
    } else if hover {
        (
            Color32::from_rgb(0x8a, 0x6d, 0xff),
            Color32::from_rgb(0xc6, 0x73, 0xff),
            Color32::from_rgb(0x3a, 0x2a, 0x8a),
        )
    } else {
        (
            Color32::from_rgb(0x6a, 0x4c, 0xff),
            Color32::from_rgb(0xa8, 0x5c, 0xf5),
            Color32::from_rgb(0x2e, 0x22, 0x70),
        )
    };
    let p = ui.painter();
    let h = 66.0 - 8.0;
    pill(p, rect.min.x + 4.0, rect.min.y + 8.0, rect.max.x - 4.0, rect.max.y, sh, sh);
    pill(p, rect.min.x + 4.0, rect.min.y + 3.0 + off, rect.max.x - 4.0, rect.min.y + h + 3.0 + off, c0, c1);
    let cy = rect.min.y + (h + 6.0) / 2.0 + off;
    let cx = rect.center().x;
    let txt_col = if enabled { Color32::WHITE } else { MUTED };
    let dy = if sub.is_empty() { 0.0 } else { 8.0 };
    p.text(pos2(cx, cy - dy), Align2::CENTER_CENTER, label, FontId::new(20.0, bold.clone()), txt_col);
    if !sub.is_empty() {
        let sc = if enabled { Color32::from_rgb(0xe6, 0xde, 0xfc) } else { MUTED };
        p.text(pos2(cx, cy + 15.0), Align2::CENTER_CENTER, sub, FontId::proportional(11.0), sc);
    }
    enabled && resp.clicked()
}

fn load_tex(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, ci, egui::TextureOptions::LINEAR))
}

fn setup_fonts(ctx: &egui::Context) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let base: Vec<String> = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut has_bold = false;
    if let Ok(b) = std::fs::read("C:\\Windows\\Fonts\\segoeuib.ttf") {
        fonts.font_data.insert("segoeb".to_string(), egui::FontData::from_owned(b));
        let mut fam = vec!["segoeb".to_string()];
        fam.extend(base.clone());
        fonts.families.insert(egui::FontFamily::Name("bold".into()), fam);
        has_bold = true;
    }
    if let Ok(b) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
        fonts.font_data.insert("segoe".to_string(), egui::FontData::from_owned(b));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.insert(0, "segoe".to_string());
        }
    }
    ctx.set_fonts(fonts);
    has_bold
}

fn setup_style(ctx: &egui::Context) {
    ctx.style_mut(|s| {
        let v = &mut s.visuals;
        v.dark_mode = true;
        v.window_fill = PANEL;
        v.panel_fill = BG;
        v.extreme_bg_color = BG;
        v.faint_bg_color = BG;
        v.selection.bg_fill = ACCENT;
        v.widgets.noninteractive.fg_stroke.color = FG;
        v.widgets.inactive.fg_stroke.color = FG;
        v.widgets.hovered.fg_stroke.color = Color32::WHITE;
        v.widgets.active.fg_stroke.color = Color32::WHITE;
        v.widgets.inactive.weak_bg_fill = BG;
        v.widgets.inactive.bg_fill = BG;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2a, 0x22, 0x58);
        v.widgets.hovered.bg_fill = Color32::from_rgb(0x2a, 0x22, 0x58);
        v.widgets.active.weak_bg_fill = ACCENT;
        v.widgets.active.bg_fill = ACCENT;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        v.window_stroke = Stroke::NONE;
        s.spacing.button_padding = vec2(8.0, 3.0);
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.0));
    });
}

// ---------------------------------------------------------------- оптимизация (фоновый поток)

#[derive(Default)]
struct Job {
    running: bool,
    finished: bool,
    frac: f32,
    log: Vec<String>,
    result: Option<String>,
    prev_scheme: Option<String>,
}

#[derive(Clone)]
struct Opts {
    close: bool,
    ram: bool,
    services: bool,
    power: bool,
    temp: bool,
    force: bool,
    auto: bool,
}

fn say(job: &Arc<Mutex<Job>>, ctx: &egui::Context, frac: f32, msg: String) {
    {
        let mut j = job.lock().unwrap();
        j.frac = frac.clamp(0.0, 1.0);
        j.log.push(msg);
    }
    ctx.request_repaint();
}

fn finish(job: &Arc<Mutex<Job>>, ctx: &egui::Context, result: String) {
    {
        let mut j = job.lock().unwrap();
        j.running = false;
        j.finished = true;
        j.frac = 1.0;
        j.result = Some(result);
    }
    ctx.request_repaint();
}

fn run_boost(o: Opts, targets: Vec<(u32, String)>, job: Arc<Mutex<Job>>, ctx: egui::Context) {
    let before = sys::ram_info().map(|r| r.1 as i64).unwrap_or(0);
    let steps = [o.close, o.services, o.power, o.temp, o.ram].iter().filter(|b| **b).count().max(1) as f32;
    let mut done = 0.0_f32;

    if o.close {
        if targets.is_empty() {
            say(&job, &ctx, done / steps, "Нет выбранных программ для закрытия".to_string());
        } else {
            say(&job, &ctx, done / steps, format!("Закрываю программы: {}", targets.len()));
            for t in &targets {
                sys::close_pid(t.0);
            }
            let t0 = Instant::now();
            loop {
                let alive = targets.iter().filter(|t| sys::is_alive(t.0)).count();
                if alive == 0 || t0.elapsed() > Duration::from_secs(6) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            let still: Vec<&(u32, String)> = targets.iter().filter(|t| sys::is_alive(t.0)).collect();
            if still.is_empty() {
                say(&job, &ctx, done / steps, "Все выбранные программы закрыты".to_string());
            } else if o.force {
                for t in &still {
                    sys::kill_pid(t.0);
                }
                say(&job, &ctx, done / steps, format!("Принудительно завершено: {}", still.len()));
            } else {
                let names: Vec<String> = still.iter().map(|t| t.1.clone()).collect();
                say(
                    &job,
                    &ctx,
                    done / steps,
                    format!("Не закрылись (в трее или ждут ответа): {}", names.join(", ")),
                );
            }
        }
        done += 1.0;
    }

    if o.services {
        say(&job, &ctx, done / steps, "Останавливаю фоновые службы...".to_string());
        let n = sys::stop_services();
        say(&job, &ctx, done / steps, format!("Остановлено служб: {} из {}", n, sys::SERVICES.len()));
        done += 1.0;
    }

    if o.power {
        say(&job, &ctx, done / steps, "Включаю схему питания «Высокая производительность»...".to_string());
        let prev = sys::current_scheme();
        if sys::set_high_performance() {
            if let Some(p) = prev {
                if p != sys::HIGH_GUID {
                    job.lock().unwrap().prev_scheme = Some(p);
                }
            }
            say(&job, &ctx, done / steps, "Схема питания: высокая производительность".to_string());
        } else {
            say(&job, &ctx, done / steps, "Не удалось переключить схему питания".to_string());
        }
        done += 1.0;
    }

    if o.temp {
        say(&job, &ctx, done / steps, "Удаляю временные файлы...".to_string());
        let bytes = sys::clean_temp();
        say(
            &job,
            &ctx,
            done / steps,
            format!("Удалено временных файлов: {:.0} МБ", bytes as f64 / 1_048_576.0),
        );
        done += 1.0;
    }

    if o.ram {
        say(&job, &ctx, done / steps, "Освобождаю оперативную память...".to_string());
        let n = sys::trim_all();
        let m = sys::purge_standby();
        let extra = if m != 0 { ", кэш Windows сброшен" } else { "" };
        say(&job, &ctx, done / steps, format!("Память очищена у процессов: {}{}", n, extra));
    }

    std::thread::sleep(Duration::from_millis(1200));
    let after = sys::ram_info().map(|r| r.1 as i64).unwrap_or(before);
    let freed = after - before;
    let head = if o.auto { "Автоочистка" } else { "Готово" };
    let msg = if freed > 20 {
        format!("{}! Свободной памяти стало больше на {} МБ", head, freed)
    } else {
        format!("{}! Память уже была в порядке", head)
    };
    finish(&job, &ctx, msg);
}

fn run_restore(prev: String, job: Arc<Mutex<Job>>, ctx: egui::Context) {
    say(&job, &ctx, 0.2, "Запускаю службы обратно...".to_string());
    let n = sys::start_services();
    say(&job, &ctx, 0.6, format!("Запущено служб: {} из {}", n, sys::SERVICES.len()));
    let ok = sys::restore_scheme(&prev);
    say(
        &job,
        &ctx,
        0.9,
        if ok { "Схема питания возвращена".to_string() } else { "Схему питания вернуть не удалось".to_string() },
    );
    finish(&job, &ctx, "Службы и схема питания возвращены как были".to_string());
}

// ---------------------------------------------------------------- приложение

enum Act {
    Boost,
    Restore,
    Settings,
    Relaunch,
}

struct App {
    ctx: egui::Context,
    cfg: sys::Config,
    job: Arc<Mutex<Job>>,
    shortcut_done: Arc<Mutex<bool>>,
    dirty: bool,
    ram: Option<(u64, u64)>,
    ram_at: Instant,
    apps: Vec<sys::AppItem>,
    apps_at: Instant,
    last_auto: Instant,
    logo: Option<egui::TextureHandle>,
    ripple: Option<(egui::Pos2, Instant)>,
    settings: bool,
    dialog: Option<(String, String)>,
    is_admin: bool,
    last_result: Option<String>,
    bold: egui::FontFamily,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, cfg: sys::Config) -> Self {
        let ctx = cc.egui_ctx.clone();
        let has_bold = setup_fonts(&ctx);
        setup_style(&ctx);
        let logo = load_tex(&ctx, "logo", include_bytes!("icon64.png"));

        // самоустановка и ярлык на рабочем столе
        let shortcut_done = Arc::new(Mutex::new(false));
        {
            let flag = shortcut_done.clone();
            let c2 = ctx.clone();
            let sv = cfg.shortcut_ver;
            std::thread::spawn(move || {
                let copied = sys::install_self();
                if (copied || sv != 1) && sys::create_shortcut() {
                    *flag.lock().unwrap() = true;
                    c2.request_repaint();
                }
            });
        }

        let mut app = App {
            ctx,
            cfg,
            job: Arc::new(Mutex::new(Job::default())),
            shortcut_done,
            dirty: false,
            ram: sys::ram_info(),
            ram_at: Instant::now(),
            apps: Vec::new(),
            apps_at: Instant::now(),
            last_auto: Instant::now(),
            logo,
            ripple: None,
            settings: false,
            dialog: None,
            is_admin: sys::is_admin(),
            last_result: None,
            bold: if has_bold { egui::FontFamily::Name("bold".into()) } else { egui::FontFamily::Proportional },
        };
        app.refresh_apps();
        app
    }

    fn bold_font(&self, size: f32) -> FontId {
        FontId::new(size, self.bold.clone())
    }

    fn is_running(&self) -> bool {
        self.job.lock().unwrap().running
    }

    fn refresh_apps(&mut self) {
        let mut list = sys::list_apps();
        for a in list.iter_mut() {
            let key = a.name.to_lowercase();
            a.selected = !self.cfg.excluded.contains(&key);
        }
        list.sort_by(|a, b| b.ws_mb.cmp(&a.ws_mb));
        self.apps = list;
        self.apps_at = Instant::now();
    }

    fn start_boost(&mut self, auto: bool) {
        {
            let mut j = self.job.lock().unwrap();
            if j.running {
                return;
            }
            j.running = true;
            j.finished = false;
            j.frac = 0.0;
            j.log.clear();
            j.result = None;
        }
        let opts = Opts {
            close: !auto && self.cfg.do_close,
            ram: auto || self.cfg.do_ram,
            services: !auto && self.cfg.do_services,
            power: !auto && self.cfg.do_power,
            temp: !auto && self.cfg.do_temp,
            force: self.cfg.force_kill,
            auto,
        };
        let targets: Vec<(u32, String)> = self
            .apps
            .iter()
            .filter(|a| a.selected)
            .map(|a| (a.pid, a.pretty_name()))
            .collect();
        let job = self.job.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || run_boost(opts, targets, job, ctx));
    }

    fn start_restore(&mut self) {
        {
            let mut j = self.job.lock().unwrap();
            if j.running {
                return;
            }
            j.running = true;
            j.finished = false;
            j.frac = 0.0;
            j.log.clear();
            j.result = None;
        }
        let prev = self.cfg.prev_scheme.clone();
        let job = self.job.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || run_restore(prev, job, ctx));
    }

    /// Забираем результаты фоновых потоков.
    fn pump(&mut self) {
        let fin = {
            let mut j = self.job.lock().unwrap();
            if j.finished {
                j.finished = false;
                Some((j.result.clone(), j.prev_scheme.take()))
            } else {
                None
            }
        };
        if let Some((res, prev)) = fin {
            if let Some(r) = res {
                self.last_result = Some(r);
            }
            if let Some(p) = prev {
                self.cfg.prev_scheme = p;
                sys::save_cfg(&self.cfg);
            }
            self.ram = sys::ram_info();
            self.ram_at = Instant::now();
            self.refresh_apps();
        }
        let sc = {
            let mut f = self.shortcut_done.lock().unwrap();
            let v = *f;
            *f = false;
            v
        };
        if sc {
            self.cfg.shortcut_ver = 1;
            sys::save_cfg(&self.cfg);
        }
    }

    fn auto_check(&mut self) {
        if !self.cfg.auto_clean || self.last_auto.elapsed() < Duration::from_secs(45) {
            return;
        }
        if self.is_running() {
            return;
        }
        if let Some((_, avail)) = self.ram {
            if (avail as u32) < self.cfg.auto_mb {
                self.last_auto = Instant::now();
                self.start_boost(true);
            }
        }
    }

    // ---------------------------------------------------------------- фон

    fn paint_bg(&self, ui: &egui::Ui, full: Rect) {
        let p = ui.painter().with_clip_rect(full);
        // стандартный фон: градиент и мягкие круги
        let n = 48;
        for i in 0..n {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            let y0 = full.top() + full.height() * t0;
            let y1 = full.top() + full.height() * t1 + 1.0;
            let c = mix(Color32::from_rgb(0x1a, 0x12, 0x3f), Color32::from_rgb(0x4a, 0x24, 0x8a), t0);
            p.rect_filled(Rect::from_min_max(pos2(full.left(), y0), pos2(full.right(), y1)), 0.0, c);
        }
        p.circle_filled(
            pos2(full.right() - 110.0, full.top() + 110.0),
            230.0,
            Color32::from_rgba_unmultiplied(0xb0, 0x5c, 0xff, 28),
        );
        p.circle_filled(
            pos2(full.left() + 120.0, full.bottom() - 40.0),
            200.0,
            Color32::from_rgba_unmultiplied(0x6a, 0x4c, 0xff, 30),
        );
    }

    // ---------------------------------------------------------------- панели

    fn left(&mut self, ui: &mut egui::Ui, act: &mut Option<Act>) {
        let fb = self.bold.clone();
        let (running, jfrac, result, log) = {
            let j = self.job.lock().unwrap();
            let tail: Vec<String> = j.log.iter().rev().take(3).rev().cloned().collect();
            (j.running, j.frac, j.result.clone(), tail)
        };
        let result = result.or_else(|| self.last_result.clone());

        card(ui, LEFT_W, |ui| {
            ui.label(RichText::new("ОПЕРАТИВНАЯ ПАМЯТЬ").font(FontId::new(11.0, fb.clone())).color(ACCENT_H));
            match self.ram {
                Some((total, avail)) => {
                    let used = total.saturating_sub(avail);
                    let frac = used as f32 / total.max(1) as f32;
                    let col = if frac < 0.6 {
                        GREEN
                    } else if frac < 0.82 {
                        AMBER
                    } else {
                        RED
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:.1} ГБ", avail as f32 / 1024.0))
                                .font(FontId::new(30.0, fb.clone()))
                                .color(Color32::WHITE),
                        );
                        ui.label(muted("свободно"));
                    });
                    bar(ui, LEFT_W - 24.0, frac, col);
                    ui.label(muted(format!("Занято {} МБ из {} МБ ({:.0}%)", used, total, frac * 100.0)));
                }
                None => {
                    ui.label(muted("Не удалось прочитать данные о памяти"));
                }
            }
        });
        ui.add_space(8.0);

        let (label, sub) = if running {
            ("РАБОТАЮ...", "подождите несколько секунд")
        } else {
            ("БУСТ", "освободить память и ускорить ноутбук")
        };
        if big_button(ui, LEFT_W, label, sub, !running, &fb) {
            *act = Some(Act::Boost);
        }
        ui.add_space(8.0);

        card(ui, LEFT_W, |ui| {
            if running {
                ui.label(RichText::new("Идёт оптимизация...").color(ACCENT_H));
                bar(ui, LEFT_W - 24.0, jfrac, ACCENT);
            } else if let Some(r) = &result {
                ui.label(RichText::new(r.as_str()).font(FontId::new(14.0, fb.clone())).color(GREEN));
            } else {
                ui.label(muted("Нажмите БУСТ: закроются выбранные программы, очистится память, включится режим скорости."));
            }
            for l in &log {
                ui.label(muted(l.as_str()).size(12.0));
            }
        });
        ui.add_space(8.0);

        card(ui, LEFT_W, |ui| {
            ui.label(RichText::new("ЧТО ДЕЛАТЬ").font(FontId::new(11.0, fb.clone())).color(ACCENT_H));
            let mut ch = false;
            ch |= ui.checkbox(&mut self.cfg.do_close, "Закрыть выбранные программы и вкладки").changed();
            ch |= ui.checkbox(&mut self.cfg.do_ram, "Очистить память и кэш Windows").changed();
            ch |= ui.checkbox(&mut self.cfg.do_services, "Выключить фоновые службы (до перезагрузки)").changed();
            ch |= ui.checkbox(&mut self.cfg.do_power, "Режим «Высокая производительность»").changed();
            ch |= ui.checkbox(&mut self.cfg.do_temp, "Удалить временные файлы").changed();
            if ch {
                sys::save_cfg(&self.cfg);
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(!running, egui::Button::new("Вернуть службы и питание").fill(BG)).clicked() {
                    *act = Some(Act::Restore);
                }
            });
        });
    }

    fn right(&mut self, ui: &mut egui::Ui, w: f32, h: f32) {
        let fb = self.bold.clone();
        let mut toggled: Vec<(String, bool)> = Vec::new();
        let (mut all, mut none, mut refresh) = (false, false, false);
        card(ui, w, |ui| {
            ui.label(RichText::new("ПРОГРАММЫ, КОТОРЫЕ ЗАКРОЮТСЯ").font(FontId::new(11.0, fb.clone())).color(ACCENT_H));
            ui.label(muted(
                "Закрываются мягко, как крестиком: если там несохранённая работа, программа спросит. \
                 Закрытие браузера закрывает все его вкладки. Снимите галочку с того, что нужно оставить.",
            ).size(12.0));
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("Выбрать все").fill(BG)).clicked() {
                    all = true;
                }
                if ui.add(egui::Button::new("Снять все").fill(BG)).clicked() {
                    none = true;
                }
                if ui.add(egui::Button::new("Обновить").fill(BG)).clicked() {
                    refresh = true;
                }
            });
            egui::ScrollArea::vertical()
                .max_height((h - 190.0).max(120.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.apps.is_empty() {
                        ui.label(muted("Открытых программ не найдено."));
                    }
                    for a in self.apps.iter_mut() {
                        let label = a.label();
                        if ui.checkbox(&mut a.selected, label).changed() {
                            toggled.push((a.name.to_lowercase(), a.selected));
                        }
                    }
                });
            let total_mb: u32 = self.apps.iter().filter(|a| a.selected).map(|a| a.ws_mb).sum();
            let cnt = self.apps.iter().filter(|a| a.selected).count();
            ui.label(muted(format!("Выбрано: {}  (занимают около {} МБ)", cnt, total_mb)));
        });

        let mut save = false;
        for (key, sel) in toggled {
            if sel {
                self.cfg.excluded.retain(|k| k != &key);
            } else if !self.cfg.excluded.contains(&key) {
                self.cfg.excluded.push(key);
            }
            save = true;
        }
        if all {
            self.cfg.excluded.clear();
            save = true;
        }
        if none {
            let mut names: Vec<String> = self.apps.iter().map(|a| a.name.to_lowercase()).collect();
            names.sort();
            names.dedup();
            self.cfg.excluded = names;
            save = true;
        }
        if save {
            sys::save_cfg(&self.cfg);
        }
        if all || none || refresh {
            self.refresh_apps();
        }
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings {
            return;
        }
        let fb = self.bold.clone();
        let mut close = false;
        let mut shortcut = false;

        egui::Window::new("settings")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .frame(egui::Frame::none().fill(PANEL).rounding(egui::Rounding::same(10.0)).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(RichText::new("НАСТРОЙКИ").font(FontId::new(11.0, fb.clone())).color(ACCENT));
                ui.add_space(6.0);

                ui.label(RichText::new("Автоочистка памяти").font(FontId::new(13.0, fb.clone())));
                if ui.checkbox(&mut self.cfg.auto_clean, "Очищать память само, когда её мало (пока Буст открыт)").changed() {
                    self.dirty = true;
                }
                if self.cfg.auto_clean {
                    let r = ui.add(egui::Slider::new(&mut self.cfg.auto_mb, 150..=1000).text("МБ свободно"));
                    if r.changed() {
                        self.dirty = true;
                    }
                }

                ui.add_space(10.0);
                ui.label(RichText::new("Закрытие программ").font(FontId::new(13.0, fb.clone())));
                if ui.checkbox(&mut self.cfg.force_kill, "Принудительно завершать то, что не закрылось").changed() {
                    self.dirty = true;
                }
                if self.cfg.force_kill {
                    ui.label(RichText::new("Осторожно: несохранённые данные в таких программах пропадут.").color(AMBER).size(12.0));
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Создать ярлык на рабочем столе").fill(BG)).clicked() {
                        shortcut = true;
                    }
                });

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("Закрыть").color(Color32::WHITE)).fill(ACCENT).min_size(vec2(100.0, 26.0)))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
            });

        if shortcut {
            sys::install_self();
            let done = sys::create_shortcut();
            self.dialog = Some((
                "Ярлык".to_string(),
                if done {
                    "Ярлык «Bnova» создан на рабочем столе.".to_string()
                } else {
                    "Не получилось создать ярлык.".to_string()
                },
            ));
        }
        if self.dirty && !ctx.input(|i| i.pointer.any_down()) {
            sys::save_cfg(&self.cfg);
            self.dirty = false;
        }
        if close {
            if self.dirty {
                sys::save_cfg(&self.cfg);
                self.dirty = false;
            }
            self.settings = false;
        }
    }

    fn dialog_ui(&mut self, ctx: &egui::Context) {
        let (title, text) = match self.dialog.clone() {
            Some(d) => d,
            None => return,
        };
        let title_font = self.bold_font(14.0);
        let mut ok = false;
        egui::Window::new("dialog")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .frame(
                egui::Frame::none()
                    .fill(PANEL)
                    .rounding(egui::Rounding::same(10.0))
                    .inner_margin(egui::Margin::same(16.0))
                    .stroke(Stroke::new(1.0, ACCENT)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.set_max_width(360.0);
                ui.label(RichText::new(title.as_str()).font(title_font).color(ACCENT_H));
                ui.add_space(6.0);
                ui.label(text.as_str());
                ui.add_space(10.0);
                if ui.add(egui::Button::new("OK").fill(ACCENT).min_size(vec2(70.0, 24.0))).clicked() {
                    ok = true;
                }
            });
        if ok {
            self.dialog = None;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump();
        if self.ram_at.elapsed() > Duration::from_millis(1500) {
            self.ram = sys::ram_info();
            self.ram_at = Instant::now();
        }
        if self.apps_at.elapsed() > Duration::from_secs(4) && !self.is_running() {
            self.refresh_apps();
        }
        self.auto_check();
        ctx.request_repaint_after(Duration::from_secs(1));

        let screen = ctx.screen_rect();
        let mut act: Option<Act> = None;

        // фон и волна от клика
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                self.paint_bg(ui, full);
                let bgr = ui.interact(full, egui::Id::new("bg"), egui::Sense::click());
                if bgr.clicked() {
                    if let Some(p) = bgr.interact_pointer_pos() {
                        self.ripple = Some((p, Instant::now()));
                    }
                }
                if let Some((pos, t0)) = self.ripple {
                    let t = t0.elapsed().as_secs_f32() / 0.256;
                    if t >= 1.0 {
                        self.ripple = None;
                    } else {
                        let rings = [(8.0 + 40.0 * t, Color32::from_rgb(0x6f, 0x4f, 0xe0)), (3.0 + 26.0 * t, Color32::WHITE)];
                        for (k, (rad, col)) in rings.iter().enumerate() {
                            let wd = (3.0 * (1.0 - t)).floor().max(1.0) + if k == 0 { 1.0 } else { 0.0 };
                            ui.painter().circle_stroke(pos, *rad, Stroke::new(wd, mix(*col, BG, t * 0.9)));
                        }
                        ctx.request_repaint();
                    }
                }
            });

        // шапка: логотип и название
        let logo_id = self.logo.as_ref().map(|t| t.id());
        let title_font = self.bold_font(28.0);
        egui::Area::new(egui::Id::new("head"))
            .fixed_pos(pos2(24.0, 16.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(id) = logo_id {
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(id, vec2(46.0, 46.0))));
                    }
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("Буст").font(title_font).color(Color32::WHITE));
                        ui.label(muted("от nova").size(12.0));
                    });
                });
            });

        // шапка справа: права и настройки
        let admin = self.is_admin;
        egui::Area::new(egui::Id::new("head_right"))
            .anchor(Align2::RIGHT_TOP, vec2(-24.0, 22.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if admin {
                        ui.label(RichText::new("Права администратора есть").color(GREEN).size(12.0));
                    } else {
                        ui.label(RichText::new("Нет прав администратора").color(AMBER).size(12.0));
                        if ui.add(egui::Button::new("Запустить от администратора").fill(BG)).clicked() {
                            act = Some(Act::Relaunch);
                        }
                    }
                    if ui.add(egui::Button::new("Настройки").fill(ACCENT).min_size(vec2(100.0, 28.0))).clicked() {
                        act = Some(Act::Settings);
                    }
                });
            });

        // левая и правая колонки
        let right_x = 24.0 + LEFT_W + 16.0;
        let right_w = (screen.width() - right_x - 24.0).max(300.0);
        let col_h = (screen.height() - 84.0 - 16.0).max(300.0);
        egui::Area::new(egui::Id::new("left"))
            .fixed_pos(pos2(24.0, 84.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                self.left(ui, &mut act);
            });
        egui::Area::new(egui::Id::new("right"))
            .fixed_pos(pos2(right_x, 84.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                self.right(ui, right_w, col_h);
            });

        self.settings_ui(ctx);
        self.dialog_ui(ctx);

        match act {
            Some(Act::Boost) => self.start_boost(false),
            Some(Act::Restore) => self.start_restore(),
            Some(Act::Settings) => self.settings = true,
            Some(Act::Relaunch) => {
                if sys::relaunch_admin() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.dialog = Some((
                        "Права администратора".to_string(),
                        "Не получилось. Закройте Буст и запустите его правой кнопкой: «Запуск от имени администратора».".to_string(),
                    ));
                }
            }
            None => {}
        }
    }
}

fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("icon64.png")).ok()?.to_rgba8();
    Some(egui::IconData { rgba: img.to_vec(), width: img.width(), height: img.height() })
}

fn main() -> eframe::Result<()> {
    let cfg = sys::load_cfg();
    let mut vp = egui::ViewportBuilder::default()
        .with_title("Буст - от nova")
        .with_inner_size([WIN_W, WIN_H])
        .with_min_inner_size([900.0, 560.0]);
    if let Some(icon) = load_icon() {
        vp = vp.with_icon(icon);
    }
    let opts = eframe::NativeOptions { viewport: vp, ..Default::default() };
    eframe::run_native("Буст", opts, Box::new(|cc| Ok(Box::new(App::new(cc, cfg)))))
}
