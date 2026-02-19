//! Game launcher for the Disobey 2026 badge.
//!
//! Shows a menu of available games and demos that can be selected and launched.
//! Controls:
//! - Up/Down: Navigate menu
//! - A: Launch selected game
//! - Select: Return to menu from any game
//!
//! This launcher provides a unified interface to access all games and demos
//! on the badge. Games are embedded as modules and launched from the menu.
//! Press SELECT at any time during a game to return to the menu.

#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use defmt::info;
#[allow(clippy::wildcard_imports)]
use disobey2026badge::*;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyle, iso_8859_1::FONT_9X18_BOLD, iso_8859_1::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use esp_backtrace as _;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use palette::Srgb;

extern crate alloc;
use alloc::vec::Vec;

esp_bootloader_esp_idf::esp_app_desc!();

const W: i32 = 320;
const H: i32 = 170;

// Global flag for returning to menu
static RETURN_TO_MENU: AtomicBool = AtomicBool::new(false);

// Menu item structure
struct MenuItem {
    name: &'static str,
    description: &'static str,
}

const MENU_ITEMS: &[MenuItem] = &[
    MenuItem { name: "Snake", description: "Classic snake game" },
    MenuItem { name: "Breakout", description: "Brick breaking game" },
    MenuItem { name: "Tetris", description: "Block stacking puzzle" },
    MenuItem { name: "Skyroads", description: "3D racing game" },
    MenuItem { name: "Space Shooter", description: "Side-scrolling shooter" },
    MenuItem { name: "Demoscene", description: "Visual effects demo" },
    MenuItem { name: "Shader", description: "Shader effects demo" },
    MenuItem { name: "Vectordemo", description: "Vector graphics demo" },
];

struct MenuState {
    selected_index: usize,
    scroll_offset: usize,
}

impl MenuState {
    fn new() -> Self {
        Self {
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    fn move_down(&mut self) {
        if self.selected_index < MENU_ITEMS.len() - 1 {
            self.selected_index += 1;
            // Show 6 items at a time
            if self.selected_index >= self.scroll_offset + 6 {
                self.scroll_offset = self.selected_index - 5;
            }
        }
    }
}

fn draw_menu(display: &mut Display, state: &MenuState) {
    // Clear screen with dark blue background
    Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::new(0, 0, 8)))
        .draw(display)
        .unwrap();

    // Draw title
    let title_style = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);
    Text::new("DISOBEY 2026", Point::new(80, 20), title_style)
        .draw(display)
        .unwrap();
    Text::new("Select Game", Point::new(100, 40), title_style)
        .draw(display)
        .unwrap();

    // Draw menu items
    let item_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let selected_style = MonoTextStyle::new(&FONT_6X10, Rgb565::BLACK);
    
    let items_to_show = 6;
    let start_y = 60;
    let item_height = 18;

    for i in 0..items_to_show {
        let item_index = state.scroll_offset + i;
        if item_index >= MENU_ITEMS.len() {
            break;
        }

        let item = &MENU_ITEMS[item_index];
        let y = start_y + (i as i32 * item_height);
        
        // Highlight selected item
        if item_index == state.selected_index {
            Rectangle::new(
                Point::new(10, y - 2),
                Size::new(300, item_height as u32),
            )
            .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
            .draw(display)
            .unwrap();
            
            Text::new(item.name, Point::new(15, y + 8), selected_style)
                .draw(display)
                .unwrap();
        } else {
            Text::new(item.name, Point::new(15, y + 8), item_style)
                .draw(display)
                .unwrap();
        }
    }

    // Draw description for selected item at bottom
    let desc_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_LIGHT_GRAY);
    let selected_item = &MENU_ITEMS[state.selected_index];
    Text::new(selected_item.description, Point::new(20, H - 10), desc_style)
        .draw(display)
        .unwrap();

    // Draw controls hint
    let hint_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_GRAY);
    Text::new("A=Launch  SELECT=Quit", Point::new(140, H - 10), hint_style)
        .draw(display)
        .unwrap();

    // Draw scroll indicators if needed
    if state.scroll_offset > 0 {
        Text::new("^", Point::new(W - 20, start_y + 8), item_style)
            .draw(display)
            .unwrap();
    }
    if state.scroll_offset + items_to_show < MENU_ITEMS.len() {
        Text::new("v", Point::new(W - 20, start_y + (items_to_show as i32 - 1) * item_height + 8), item_style)
            .draw(display)
            .unwrap();
    }
}

// ============================================================================
// SNAKE GAME MODULE (example of embedded game)
// ============================================================================

mod snake {
    use super::*;

    const GRID_SIZE: i32 = 10;
    const GRID_W: i32 = W / GRID_SIZE;
    const GRID_H: i32 = H / GRID_SIZE;
    const TICK_MS: u64 = 100;

    const SNAKE_COLOR: Rgb565 = Rgb565::GREEN;
    const FOOD_COLOR: Rgb565 = Rgb565::RED;

    // Simple RNG
    struct Rng(u32);
    impl Rng {
        const fn new(seed: u32) -> Self { Self(seed) }
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0
        }
        fn range(&mut self, max: u32) -> u32 { self.next() % max }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Pos {
        x: i32,
        y: i32,
    }

    #[derive(Clone, Copy, Debug)]
    enum Direction {
        Up,
        Down,
        Left,
        Right,
    }

    impl Direction {
        fn is_opposite(self, other: Direction) -> bool {
            matches!(
                (self, other),
                (Direction::Up, Direction::Down)
                    | (Direction::Down, Direction::Up)
                    | (Direction::Left, Direction::Right)
                    | (Direction::Right, Direction::Left)
            )
        }
    }

    struct Game {
        snake: Vec<Pos>,
        direction: Direction,
        next_direction: Direction,
        food: Pos,
        score: u16,
        game_over: bool,
        rng: Rng,
    }

    impl Game {
        fn new() -> Self {
            let mut game = Self {
                snake: Vec::new(),
                direction: Direction::Right,
                next_direction: Direction::Right,
                food: Pos { x: 0, y: 0 },
                score: 0,
                game_over: false,
                rng: Rng::new(12345),
            };

            // Start with 3-segment snake in the middle
            let mid_x = GRID_W / 2;
            let mid_y = GRID_H / 2;
            game.snake.push(Pos { x: mid_x, y: mid_y });
            game.snake.push(Pos { x: mid_x - 1, y: mid_y });
            game.snake.push(Pos { x: mid_x - 2, y: mid_y });

            game.spawn_food();
            game
        }

        fn spawn_food(&mut self) {
            loop {
                let food = Pos {
                    x: (self.rng.range(GRID_W as u32) as i32),
                    y: (self.rng.range(GRID_H as u32) as i32),
                };

                if !self.snake.contains(&food) {
                    self.food = food;
                    break;
                }
            }
        }

        fn tick(&mut self) {
            if self.game_over {
                return;
            }

            // Update direction (but prevent reversing)
            if !self.direction.is_opposite(self.next_direction) {
                self.direction = self.next_direction;
            }

            // Calculate new head position
            let head = self.snake[0];
            let new_head = match self.direction {
                Direction::Up => Pos { x: head.x, y: head.y - 1 },
                Direction::Down => Pos { x: head.x, y: head.y + 1 },
                Direction::Left => Pos { x: head.x - 1, y: head.y },
                Direction::Right => Pos { x: head.x + 1, y: head.y },
            };

            // Check wall collision
            if new_head.x < 0 || new_head.x >= GRID_W || new_head.y < 0 || new_head.y >= GRID_H {
                self.game_over = true;
                return;
            }

            // Check self collision
            if self.snake.contains(&new_head) {
                self.game_over = true;
                return;
            }

            // Move snake
            self.snake.insert(0, new_head);

            // Check food collision
            if new_head == self.food {
                self.score += 10;
                self.spawn_food();
            } else {
                self.snake.pop();
            }
        }
    }

    fn draw_title(display: &mut Display) {
        Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(display)
            .unwrap();

        let title_style = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::GREEN);
        Text::new("SNAKE", Point::new(120, 70), title_style)
            .draw(display)
            .unwrap();

        let msg_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        Text::new("Press A to start", Point::new(100, 100), msg_style)
            .draw(display)
            .unwrap();
    }

    fn draw_initial(display: &mut Display, game: &Game) {
        Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(display)
            .unwrap();

        draw_snake(display, game);
        draw_food(display, game);
        draw_score(display, game.score);
    }

    fn draw_snake(display: &mut Display, game: &Game) {
        for segment in &game.snake {
            Rectangle::new(
                Point::new(segment.x * GRID_SIZE, segment.y * GRID_SIZE),
                Size::new(GRID_SIZE as u32 - 1, GRID_SIZE as u32 - 1),
            )
            .into_styled(PrimitiveStyle::with_fill(SNAKE_COLOR))
            .draw(display)
            .unwrap();
        }
    }

    fn draw_food(display: &mut Display, game: &Game) {
        Rectangle::new(
            Point::new(game.food.x * GRID_SIZE, game.food.y * GRID_SIZE),
            Size::new(GRID_SIZE as u32 - 1, GRID_SIZE as u32 - 1),
        )
        .into_styled(PrimitiveStyle::with_fill(FOOD_COLOR))
        .draw(display)
        .unwrap();
    }

    fn draw_score(display: &mut Display, score: u16) {
        let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let mut buf = [0u8; 24];
        let score_str = format_score(score, &mut buf);
        
        // Clear score area
        Rectangle::new(Point::new(0, 0), Size::new(100, 12))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(display)
            .unwrap();
        
        Text::new(score_str, Point::new(4, 10), style)
            .draw(display)
            .unwrap();
    }

    fn draw_game_over(display: &mut Display, score: u16) {
        let bg_style = PrimitiveStyle::with_fill(Rgb565::new(10, 0, 0));
        Rectangle::new(Point::new(60, 60), Size::new(200, 50))
            .into_styled(bg_style)
            .draw(display)
            .unwrap();

        let title_style = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::RED);
        Text::new("GAME OVER", Point::new(80, 80), title_style)
            .draw(display)
            .unwrap();

        let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let mut buf = [0u8; 24];
        let score_str = format_score(score, &mut buf);
        Text::new(score_str, Point::new(120, 100), style)
            .draw(display)
            .unwrap();
    }

    fn format_u16(mut n: u16, buf: &mut [u8; 16]) -> &str {
        if n == 0 {
            buf[0] = b'0';
            return unsafe { core::str::from_utf8_unchecked(&buf[..1]) };
        }
        let mut i = 0;
        let mut tmp = [0u8; 5];
        while n > 0 {
            tmp[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        for j in 0..i {
            buf[j] = tmp[i - 1 - j];
        }
        unsafe { core::str::from_utf8_unchecked(&buf[..i]) }
    }

    fn format_score(score: u16, buf: &mut [u8; 24]) -> &str {
        let prefix = b"Score: ";
        buf[..prefix.len()].copy_from_slice(prefix);
        let mut num_buf = [0u8; 16];
        let num_str = format_u16(score, &mut num_buf);
        let num_bytes = num_str.as_bytes();
        buf[prefix.len()..prefix.len() + num_bytes.len()].copy_from_slice(num_bytes);
        let total = prefix.len() + num_bytes.len();
        unsafe { core::str::from_utf8_unchecked(&buf[..total]) }
    }

    fn update_leds(leds: &mut Leds, game: &Game) {
        if game.game_over {
            leds.fill(Srgb::new(20, 0, 0));
        } else {
            // Show score as LED bar graph
            let lit = (game.score as usize / 10).min(BAR_COUNT);
            let mut left = [Srgb::new(0u8, 0, 0); BAR_COUNT];
            let mut right = [Srgb::new(0u8, 0, 0); BAR_COUNT];

            for i in 0..lit {
                let color = Srgb::new(0, 10, 0);
                if i < BAR_COUNT / 2 {
                    left[i] = color;
                } else {
                    right[i - BAR_COUNT / 2] = color;
                }
            }
            leds.set_left_bar(&left);
            leds.set_right_bar(&right);
        }
    }

    pub async fn run(
        display: &mut Display<'_>,
        backlight: &mut Backlight,
        leds: &mut Leds<'_>,
        buttons: &mut Buttons,
    ) {
        info!("Snake game started");
        backlight.on();

        // Title screen
        draw_title(display);
        leds.clear();
        leds.update().await;

        // Wait for A press to start
        loop {
            let pressed = embassy_futures::select::select_array([
                Buttons::debounce_press(&mut buttons.a),
                Buttons::debounce_press(&mut buttons.select),
            ])
            .await;

            if pressed.1 == 1 {
                // Select pressed - return to menu
                info!("Returning to menu from title");
                return;
            } else {
                // A pressed - start game
                break;
            }
        }

        // Game loop
        let mut game = Game::new();
        draw_initial(display, &game);
        let tick = Duration::from_millis(TICK_MS);

        loop {
            // Check for return to menu
            if RETURN_TO_MENU.load(Ordering::Relaxed) {
                info!("Returning to menu from game");
                RETURN_TO_MENU.store(false, Ordering::Relaxed);
                return;
            }

            // Poll d-pad for next direction
            if buttons.up.is_low() {
                game.next_direction = Direction::Up;
            } else if buttons.down.is_low() {
                game.next_direction = Direction::Down;
            } else if buttons.left.is_low() {
                game.next_direction = Direction::Left;
            } else if buttons.right.is_low() {
                game.next_direction = Direction::Right;
            }

            game.tick();
            draw_snake(display, &game);
            draw_food(display, &game);
            draw_score(display, game.score);
            update_leds(leds, &game);
            leds.update().await;

            if game.game_over {
                Timer::after(Duration::from_millis(500)).await;
                draw_game_over(display, game.score);

                // Flash LEDs for game over
                for _ in 0..3 {
                    leds.fill(Srgb::new(20, 0, 0));
                    leds.update().await;
                    Timer::after(Duration::from_millis(300)).await;
                    leds.clear();
                    leds.update().await;
                    Timer::after(Duration::from_millis(300)).await;
                }

                // Wait for A to restart or Select to quit
                loop {
                    let pressed = embassy_futures::select::select_array([
                        Buttons::debounce_press(&mut buttons.a),
                        Buttons::debounce_press(&mut buttons.select),
                    ])
                    .await;

                    if pressed.1 == 1 {
                        // Select pressed - return to menu
                        info!("Returning to menu after game over");
                        return;
                    } else {
                        // A pressed - restart game
                        info!("Restarting Snake");
                        game = Game::new();
                        draw_initial(display, &game);
                        break;
                    }
                }
            }

            Timer::after(tick).await;
        }
    }
}

// ============================================================================
// Placeholder functions for other games (to be implemented)
// ============================================================================

async fn run_breakout(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Breakout", leds, buttons).await;
}

async fn run_tetris(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Tetris", leds, buttons).await;
}

async fn run_skyroads(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Skyroads", leds, buttons).await;
}

async fn run_space_shooter(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Space Shooter", leds, buttons).await;
}

async fn run_demoscene(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Demoscene", leds, buttons).await;
}

async fn run_shader(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Shader", leds, buttons).await;
}

async fn run_vectordemo(
    display: &mut Display<'_>,
    _backlight: &mut Backlight,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    show_coming_soon(display, "Vectordemo", leds, buttons).await;
}

async fn show_coming_soon(
    display: &mut Display<'_>,
    name: &str,
    leds: &mut Leds<'_>,
    buttons: &mut Buttons,
) {
    Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
        .draw(display)
        .unwrap();

    let title_style = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);
    Text::new(name, Point::new(120, 60), title_style)
        .draw(display)
        .unwrap();

    let msg_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_GRAY);
    Text::new("Coming soon!", Point::new(110, 90), msg_style)
        .draw(display)
        .unwrap();
    Text::new("Press SELECT to return", Point::new(80, 110), msg_style)
        .draw(display)
        .unwrap();

    // Pulse LEDs
    for _ in 0..10 {
        for i in 0..10 {
            leds.set(i, Srgb::new(5, 5, 10));
        }
        leds.update().await;
        Timer::after(Duration::from_millis(300)).await;

        leds.clear();
        leds.update().await;
        Timer::after(Duration::from_millis(300)).await;

        // Check for select button
        if buttons.select.is_low() {
            Buttons::debounce_press(&mut buttons.select).await;
            return;
        }
    }

    Buttons::debounce_press(&mut buttons.select).await;
}

// ============================================================================
// Main launcher task
// ============================================================================

#[embassy_executor::task]
async fn launcher_task(
    display: &'static mut Display<'static>,
    backlight: &'static mut Backlight,
    leds: &'static mut Leds<'static>,
    buttons_for_select: &'static mut Buttons,
) {
    // Create button resources for menu and games
    // Note: In a real implementation, you'd need separate button instances
    // or use a different pattern. For now, we'll use polling.
    
    // Turn on backlight
    backlight.on();

    // Initialize LEDs to a pleasant blue glow
    for i in 0..10 {
        leds.set(i, Srgb::new(0, 5, 15));
    }
    leds.write();

    let mut state = MenuState::new();
    draw_menu(display, &state);

    info!("Launcher ready - {} games available", MENU_ITEMS.len());

    loop {
        // Wait a bit and check buttons
        Timer::after(Duration::from_millis(50)).await;

        // Check button states
        if buttons_for_select.up.is_low() {
            Buttons::debounce_press(&mut buttons_for_select.up).await;
            state.move_up();
            draw_menu(display, &state);
            info!("Menu up: {}", state.selected_index);
        } else if buttons_for_select.down.is_low() {
            Buttons::debounce_press(&mut buttons_for_select.down).await;
            state.move_down();
            draw_menu(display, &state);
            info!("Menu down: {}", state.selected_index);
        } else if buttons_for_select.a.is_low() {
            Buttons::debounce_press(&mut buttons_for_select.a).await;
            
            let game_name = MENU_ITEMS[state.selected_index].name;
            info!("Launching: {}", game_name);
            
            // Flash LEDs to indicate selection
            for _ in 0..2 {
                for i in 0..10 {
                    leds.set(i, Srgb::new(0, 20, 0));
                }
                leds.write();
                Timer::after(Duration::from_millis(100)).await;
                
                for i in 0..10 {
                    leds.set(i, Srgb::new(0, 0, 0));
                }
                leds.write();
                Timer::after(Duration::from_millis(100)).await;
            }
            
            // Launch the selected game
            RETURN_TO_MENU.store(false, Ordering::Relaxed);
            
            match state.selected_index {
                0 => snake::run(display, backlight, leds, buttons_for_select).await,
                1 => run_breakout(display, backlight, leds, buttons_for_select).await,
                2 => run_tetris(display, backlight, leds, buttons_for_select).await,
                3 => run_skyroads(display, backlight, leds, buttons_for_select).await,
                4 => run_space_shooter(display, backlight, leds, buttons_for_select).await,
                5 => run_demoscene(display, backlight, leds, buttons_for_select).await,
                6 => run_shader(display, backlight, leds, buttons_for_select).await,
                7 => run_vectordemo(display, backlight, leds, buttons_for_select).await,
                _ => {}
            }
            
            // Return to menu
            info!("Returned to launcher menu");
            draw_menu(display, &state);
            
            // Reset LED glow
            for i in 0..10 {
                leds.set(i, Srgb::new(0, 5, 15));
            }
            leds.write();
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let peripherals = disobey2026badge::init();
    let resources = split_resources!(peripherals);

    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let display = mk_static!(Display<'static>, resources.display.into());
    let backlight = mk_static!(Backlight, resources.backlight.into());
    let leds = mk_static!(Leds<'static>, resources.leds.into());
    let buttons = mk_static!(Buttons, resources.buttons.into());

    spawner.must_spawn(launcher_task(display, backlight, leds, buttons));

    loop {
        Timer::after(Duration::from_secs(600)).await;
    }
}
