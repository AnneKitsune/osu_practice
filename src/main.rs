use amethyst::core::frame_limiter::FrameRateLimitStrategy;
use amethyst::ecs::*;
use amethyst::prelude::*;
use amethyst::shrev::{EventChannel, ReaderId};
use amethyst::utils::*;
use amethyst::utils::circular_buffer::*;
use easycurses::*;
use std::collections::HashMap;
use std::time::*;
use lazy_static::lazy_static;

pub struct Curses(pub EasyCurses);

#[derive(Default)]
pub struct Stats {
    pub total: u32,
    pub combo: u32,
    pub score: u64,
}

// boi
unsafe impl Send for Curses {}
// Garanteed by the system execution scheduler
unsafe impl Sync for Curses {}

lazy_static! {
    static ref COLOR_NORMAL: easycurses::ColorPair = easycurses::ColorPair::new(Color::White, Color::Black);
    static ref COLOR_EDGE: easycurses::ColorPair = easycurses::ColorPair::new(Color::Yellow, Color::Black);
    static ref COLOR_TITLE: easycurses::ColorPair = easycurses::ColorPair::new(Color::Red, Color::White);
    static ref COLOR_DEBUG: easycurses::ColorPair = easycurses::ColorPair::new(Color::Blue, Color::White);
}

pub struct CursesRenderSystem;

impl<'a> System<'a> for CursesRenderSystem {
    type SystemData = (
        WriteExpect<'a, Curses>,
        ReadExpect<'a, CircularBuffer<Instant>>,
        Read<'a, Stats>,
    );
    fn run(&mut self, (mut curses, buf, stats): Self::SystemData) {
        let curses = &mut curses.0;

        // Clear the screen
        curses.set_color_pair(*COLOR_NORMAL);
        for y in 0..100 {
            for x in 0..100 {
                curses.move_rc(y as i32, x as i32);
                curses.print_char(' ');
            }
        }

        if let Some(start) = buf.queue().front() {
            let mut avg: f64 = buf.queue().iter().skip(1).map(|e| e.duration_since(*start).as_secs_f64()).sum();
            if avg > 0.01 {
                avg = avg / (buf.queue().len() - 1) as f64;
            }
            curses.move_rc(0, 0);
            curses.print(format!("Average delay between presses: {}", avg));
            curses.move_rc(1, 0);
            curses.print(format!("KPS: {}", 1.0/avg));
            curses.move_rc(2, 0);
            curses.print(format!("BPM: {}", (1.0/avg) * 60.0));

            curses.move_rc(4, 0);
            curses.print(format!("Total Presses: {}", stats.total));
            curses.move_rc(5, 0);
            curses.print(format!("Combo: {}", stats.combo));
            curses.move_rc(6, 0);
            curses.print(format!("Score: {}", stats.score));
        }

        // Render
        curses.refresh();
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InputEvent {
    Input,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Keymap {
    pub map: HashMap<Input, InputEvent>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap {
            map: [
                (Input::Character('x'), InputEvent::Input),
                (Input::Character('b'), InputEvent::Input),
            ].iter().cloned().collect(),
        }
    }
}

pub struct CursesInputSystem;

impl<'a> System<'a> for CursesInputSystem {
    type SystemData = (
        Write<'a, EventChannel<InputEvent>>,
        WriteExpect<'a, Curses>,
        Read<'a, Keymap>,
    );
    fn run(&mut self, (mut input_ev, mut curses, keymap): Self::SystemData) {
        let curses = &mut curses.0;
        while let Some(input) = curses.get_input() {
            if let Some(ev) = keymap.map.get(&input) {
                input_ev.single_write(*ev);
            }
        }
    }
}

#[derive(Default)]
pub struct OsuInputSystem {
    reader: Option<ReaderId<InputEvent>>,
}

impl<'a> System<'a> for OsuInputSystem {
    type SystemData = (
        Write<'a, EventChannel<InputEvent>>,
        Write<'a, Stats>,
        WriteExpect<'a, CircularBuffer<Instant>>,
    );
    fn run(&mut self, (mut input_ev, mut stats, mut buf): Self::SystemData) {
        if self.reader.is_none() {
            self.reader = Some(input_ev.register_reader());
        }
        for ev in input_ev.read(&mut self.reader.as_mut().unwrap()) {
            match ev {
                InputEvent::Input => {
                    stats.total += 1;
                    if let Some(delay) = buf.queue().back() {
                        if Instant::now().duration_since(*delay).as_secs_f32() > 1.0 {
                            stats.combo = 0;
                        }
                    }
                    buf.push(Instant::now());
                    stats.combo += 1;
                    stats.score += stats.combo as u64;
                },
            }
        }
    }
}

pub struct InitState;

impl SimpleState for InitState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        println!("Game started!");

        let mut curses = EasyCurses::initialize_system().expect("Failed to start ncurses.");
        curses.set_input_mode(InputMode::Character);
        curses.set_keypad_enabled(true);
        curses.set_echo(false);
        curses.set_cursor_visibility(CursorVisibility::Invisible);
        curses.set_input_timeout(TimeoutMode::Immediate);
        #[cfg(unix)]
        unsafe{ ncurses::ll::set_escdelay(0) };

        curses.refresh();

        data.world.insert(Curses(curses));
        data.world.insert(CircularBuffer::<Instant>::new(8));
    }

    fn update(&mut self, _data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        Trans::None
    }
}

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let app_root = application_root_dir()?;
    let assets_dir = app_root.join("assets/");

    let game_data = GameDataBuilder::default()
        .with(CursesInputSystem, "curses_input", &[])
        .with(OsuInputSystem::default(), "osu_input", &["curses_input"])
        .with(CursesRenderSystem, "curses_render", &["osu_input"]);
    let mut game = Application::build(assets_dir, InitState)?
        .with_frame_limit(
            FrameRateLimitStrategy::SleepAndYield(Duration::from_millis(2)),
            60,
        )
        .build(game_data)?;
    game.run();
    Ok(())
}

