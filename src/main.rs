mod nodes;
use nodes::Nodes;

fn entry_yaml_bytes() -> Vec<u8> {
    std::fs::read("entry.yaml").expect("couldn't read file entry.yaml")
}

fn window_init() -> macroquad::Conf {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WindowConfig {
        title: String,
        height: i32,
        width: i32,
        fullscreen: bool,
    }

    #[derive(serde::Deserialize)]
    struct Config {
        window: WindowConfig,
    }

    let Config { window } = serde_yaml::from_slice(&entry_yaml_bytes()).unwrap();
    macroquad::Conf {
        window_title: window.title,
        window_width: window.width,
        window_height: window.height,
        fullscreen: window.fullscreen,
        ..Default::default()
    }
}

#[macroquad::main(window_init)]
async fn main() {
    use macroquad::*;

    let mut nodes = Nodes::new();

    loop {
        nodes.update();
        nodes.render();
        next_frame().await;
    }
}
