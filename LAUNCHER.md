# Launcher Implementation Guide

## Overview

The launcher (`examples/launcher.rs`) provides a unified menu interface for accessing all games and demos on the Disobey 2026 badge. Users can navigate through available games using the D-pad and launch them with the A button.

## Features

- **Menu Navigation**: Use Up/Down on the D-pad to browse through games
- **Game Selection**: Press A to launch the selected game
- **Return to Menu**: Press SELECT at any time during a game to return to the launcher menu
- **Visual Feedback**: LED animations indicate menu selection and game states
- **Scrolling Menu**: Displays 6 items at a time with scroll indicators

## Current Status

- **Fully Integrated**: Snake game
- **Placeholders**: Breakout, Tetris, Skyroads, Space Shooter, Demoscene, Shader, Vectordemo

## How to Add a Game to the Launcher

### Step 1: Create a Game Module

Add your game as a module in `launcher.rs`. The game should be structured as a standalone module:

```rust
mod your_game {
    use super::*;
    
    // Your game constants, structs, and logic here
    
    pub async fn run(
        display: &mut Display<'_>,
        backlight: &mut Backlight,
        leds: &mut Leds<'_>,
        buttons: &mut Buttons,
    ) {
        // Game initialization
        backlight.on();
        
        // Main game loop
        loop {
            // Check for return to menu
            if RETURN_TO_MENU.load(Ordering::Relaxed) {
                RETURN_TO_MENU.store(false, Ordering::Relaxed);
                return;
            }
            
            // Game logic here
            
            // Handle game over
            if game_over {
                // Wait for A to restart or SELECT to quit
                // Return from function to go back to menu
                return;
            }
        }
    }
}
```

### Step 2: Add Menu Entry

Add your game to the `MENU_ITEMS` array:

```rust
const MENU_ITEMS: &[MenuItem] = &[
    // ... existing items ...
    MenuItem { name: "Your Game", description: "Game description" },
];
```

### Step 3: Add Launch Handler

In the `launcher_task` function, add a case for your game in the match statement:

```rust
match state.selected_index {
    0 => snake::run(display, backlight, leds, buttons_for_select).await,
    1 => run_breakout(display, backlight, leds, buttons_for_select).await,
    // ... existing cases ...
    N => your_game::run(display, backlight, leds, buttons_for_select).await,
    _ => {}
}
```

### Step 4: Handle Return to Menu

Your game should check the `RETURN_TO_MENU` atomic flag regularly:

```rust
if RETURN_TO_MENU.load(Ordering::Relaxed) {
    RETURN_TO_MENU.store(false, Ordering::Relaxed);
    return;
}
```

Or provide a "return to menu" option in game over or pause screens:

```rust
// Wait for button press
let pressed = embassy_futures::select::select_array([
    Buttons::debounce_press(&mut buttons.a),
    Buttons::debounce_press(&mut buttons.select),
])
.await;

if pressed.1 == 1 {
    // Select pressed - return to menu
    return;
}
```

## Snake Game Example

The Snake game is fully integrated and serves as a reference implementation. Key features:

- **Title Screen**: Shows game name and waits for A button
- **Game Loop**: Updates at 100ms intervals, checks for return to menu
- **LED Feedback**: Shows score progression as LED bar graph
- **Game Over**: Offers restart (A) or return to menu (SELECT)
- **Clean Exit**: Returns control to launcher when done

## Design Patterns

### Shared Resources

Games receive mutable references to peripherals:
- `display`: For rendering graphics
- `backlight`: For display backlight control
- `leds`: For LED effects
- `buttons`: For input handling

### Async Game Loop

Games use async/await for timing and input:
```rust
loop {
    // Game logic
    Timer::after(Duration::from_millis(tick_rate)).await;
}
```

### Button Polling vs. Waiting

- **Polling**: Check `button.is_low()` for real-time input (movement)
- **Waiting**: Use `Buttons::debounce_press()` for menu selections

### Memory Management

- Use `alloc::vec::Vec` for dynamic collections (snake body, etc.)
- Keep stack usage reasonable for embedded environment
- Set heap size appropriately: `esp_alloc::heap_allocator!(size: 128 * 1024);`

## Building and Running

```bash
# Build the launcher
cargo build --release --example launcher

# Flash to badge
cargo run --release --example launcher
```

## Testing Without Hardware

While the launcher requires ESP32-S3 hardware and toolchain to run, you can:
1. Verify syntax and basic compilation
2. Review code structure and logic
3. Test individual game modules as separate examples first

## Future Enhancements

Potential improvements to the launcher system:

1. **Persistent Settings**: Save last selected game
2. **High Score Tracking**: Share scores across launcher restarts  
3. **Game Categories**: Organize into Games, Demos, Utilities
4. **Thumbnails**: Show small game preview images
5. **Smooth Transitions**: Add fade effects between menu and games
6. **Multi-Page Menus**: Support more than 8 entries with pagination
7. **Search/Filter**: Quick game selection by first letter

## Troubleshooting

### Game Doesn't Return to Menu

- Ensure `RETURN_TO_MENU` flag is checked in the main loop
- Add SELECT button handler in game over/pause screens
- Verify function returns properly to transfer control back

### Button Conflicts

- The SELECT button is monitored globally for return-to-menu
- Don't use SELECT for game-specific actions
- Use A/B for primary/secondary actions in games

### Memory Issues

- Increase heap size if needed: `esp_alloc::heap_allocator!(size: N * 1024);`
- Profile memory usage during game execution
- Reduce allocations in tight loops

### Display Artifacts

- Clear screen when entering/exiting game: `Rectangle::new().into_styled(black).draw()`
- Reset LED state when returning to menu
- Restore backlight state if modified

## Contributing

To contribute a game integration:

1. Fork the repository
2. Add your game module following the patterns above
3. Test thoroughly on hardware
4. Submit a pull request with:
   - Game module code
   - Menu entry addition
   - README update
   - Demo screenshot/video (if possible)

## License

This launcher and all game integrations follow the repository's MIT license.
