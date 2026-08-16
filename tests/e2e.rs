use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

// Requires a built binary. Run this with `cargo test --test e2e`

#[test]
fn test_e2e_workflow() {
    // 0. Kill any existing timezone-picker processes that might hold the hotkey
    let _ = Command::new("powershell")
        .args([
            "-command",
            "Stop-Process -Name timezone-picker -Force -ErrorAction SilentlyContinue",
        ])
        .status();

    // 1. Start the timezone-picker app
    let mut app = Command::new("cargo")
        .args(["run", "--release"])
        .spawn()
        .expect("Failed to start timezone-picker");

    // Wait for it to boot and register hotkey
    sleep(Duration::from_secs(3));

    // 2. Start Dummy UI Window perfectly centered with large font
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$form = New-Object System.Windows.Forms.Form
$form.StartPosition = 'CenterScreen'
$form.Width = 600
$form.Height = 300
$form.TopMost = $true
$tb = New-Object System.Windows.Forms.TextBox
$tb.Text = 'Aug 15, 2026 3:30 PM PST to EST'
$tb.Dock = 'Fill'
$tb.Multiline = $true
$tb.Font = New-Object System.Drawing.Font('Consolas', 24)
$form.Controls.Add($tb)
$form.ShowDialog()
"#;
    let mut ui_window = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", script])
        .spawn()
        .expect("Failed to start dummy window");

    // Wait for window to render
    sleep(Duration::from_secs(3));

    let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();

    // 4. Trigger the global hotkey (Ctrl + Alt + Z)
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::Alt, Direction::Press);
    let _ = enigo.key(Key::Unicode('z'), Direction::Click);
    let _ = enigo.key(Key::Alt, Direction::Release);
    let _ = enigo.key(Key::Control, Direction::Release);

    // Wait for the overlay to appear
    sleep(Duration::from_millis(1000));

    // 5. Simulate mouse drag perfectly across the center of the screen
    // Since our custom form is exactly centered and 600x300, a drag near the center is guaranteed to hit it.
    let (screen_w, screen_h) = enigo.main_display().unwrap();
    let cx = screen_w / 2;
    let cy = screen_h / 2;

    let _ = enigo.move_mouse(cx - 200, cy - 50, Coordinate::Abs);
    let _ = enigo.button(Button::Left, Direction::Press);
    let _ = enigo.move_mouse(cx + 200, cy + 50, Coordinate::Abs);
    let _ = enigo.button(Button::Left, Direction::Release);

    // Wait for processing and popup
    sleep(Duration::from_secs(2));

    // 6. Press Enter to copy the conversion from the interactive popup
    let _ = enigo.key(Key::Return, Direction::Click);
    sleep(Duration::from_millis(500));

    // 7. Verify clipboard output
    let mut success = false;
    // Retry clipboard reading a few times in case the app is still saving it
    for _ in 0..10 {
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-command", "Get-Clipboard"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("06:30 PM") {
                success = true;
                break;
            }
        }
        sleep(Duration::from_millis(200));
    }

    assert!(
        success,
        "Expected 06:30 PM in output but couldn't find it or clipboard read failed"
    );

    app.kill().ok();
    let _ = app.wait();
    ui_window.kill().ok();
    let _ = ui_window.wait();
}
