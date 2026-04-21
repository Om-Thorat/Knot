use eframe::egui;
use evdev::uinput::VirtualDevice;
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, Key, RelativeAxisType, InputEvent, EventType, Device,AbsoluteAxisType};
use std::sync::{Arc, Mutex};
use std::thread;

struct KnotApp {
    pos: Arc<Mutex<egui::Pos2>>,
    // device: Arc<Mutex<VirtualDevice>>,
}

// Adjust these to your actual screen resolution
const SCREEN_WIDTH: i32 = 2880;
const SCREEN_HEIGHT: i32 = 1800;

impl KnotApp {
    fn new(cc: &eframe::CreationContext<'_>, device: VirtualDevice) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_shadow = egui::epaint::Shadow::NONE;
        cc.egui_ctx.set_visuals(visuals);

        let pos = Arc::new(Mutex::new(egui::pos2(500.0, 500.0)));
        let device = Arc::new(Mutex::new(device));

        // SPAWN THE PHYSICAL MOUSE LISTENER
        let pos_clone = Arc::clone(&pos);
        let device_clone = Arc::clone(&device);
        
        thread::spawn(move || {
            // Open your Logi M650 (ensure path is correct via libinput list-devices)
            let mut bt_mouse = Device::open("/dev/input/event11").expect("Failed to open Logi M650");
            
            loop {
                match bt_mouse.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            // 1. UPDATE VISUAL POSITION (White Cursor)
                            if let evdev::InputEventKind::RelAxis(axis) = event.kind() {
                                let val = event.value();
                                let mut p = pos_clone.lock().unwrap();

                                match axis {
                                    RelativeAxisType::REL_X => p.x = (p.x + val as f32).clamp(0.0, SCREEN_WIDTH as f32),
                                    RelativeAxisType::REL_Y => p.y = (p.y + val as f32).clamp(0.0, SCREEN_HEIGHT as f32),
                                    _ => {}
                                }
                            }

                            // 2. HANDLE CLICKS (Emit relative movement + click)
                            if let evdev::InputEventKind::Key(key) = event.kind() {
                                let val = event.value();
                                let p = pos_clone.lock().unwrap();
                                let mut d = device_clone.lock().unwrap();
                                
                                d.emit(&[
                                    InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, p.x as i32),
                                    InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, p.y as i32),
                                    InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)
                                ]).unwrap();

                                // Emit click event
                                d.emit(&[
                                    InputEvent::new(EventType::KEY, key.0, val),
                                    InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
                                ]).unwrap();
                                
                                println!("Click event: key={:?}, val={}", key, val);
                            }
                        }
                    },
                    Err(_) => break,
                }
            }
        });

        Self { pos }
    }
}

impl eframe::App for KnotApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // Total transparency
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let pos = *self.pos.lock().unwrap();

        // DRAW THE CURSOR (Arrow)
        let color = egui::Color32::WHITE;
        let stroke = egui::Stroke::new(1.5, egui::Color32::BLACK);

        let points = vec![
            pos,
            pos + egui::vec2(0.0, 18.0),
            pos + egui::vec2(5.0, 14.0),
            pos + egui::vec2(12.0, 12.0),
        ];
        
        painter.add(egui::Shape::convex_polygon(points, color, stroke));

        // Request constant repaint to stay in sync with the hardware thread
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    // Initialize uinput device for seat1
    let mut keys = AttributeSet::<Key>::new();
    keys.insert(Key::BTN_LEFT);
    keys.insert(Key::BTN_RIGHT);
    keys.insert(Key::BTN_MIDDLE);
    
    let device = VirtualDeviceBuilder::new().unwrap()
        .name("Knot-Secondary-Mouse")
        .with_keys(&keys).unwrap()
        .with_relative_axes(&AttributeSet::from_iter([RelativeAxisType::REL_X, RelativeAxisType::REL_Y])).unwrap()
        .build().unwrap();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_mouse_passthrough(true)
            .with_always_on_top()
            .with_fullscreen(true)
            .with_decorations(false),
        renderer: eframe::Renderer::Glow, 
        ..Default::default()
    };

    eframe::run_native(
        "Knot Overlay",
        options,
        Box::new(|cc| Box::new(KnotApp::new(cc, device))),
    )
}