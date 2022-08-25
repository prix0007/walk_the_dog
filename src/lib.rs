use engine::GameLoop;
use engine::KeyState;
use engine::Point;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlImageElement;

use std::collections::HashMap;

#[macro_use]
mod browser;
mod engine;

use crate::engine::{Game, Rect, Renderer};
use anyhow::Result;
use async_trait::async_trait;
// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Deserialize)]
struct SheetRect {
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}
#[derive(Deserialize)]
struct Cell {
    frame: SheetRect,
}
#[derive(Deserialize)]
pub struct Sheet {
    frames: HashMap<String, Cell>,
}
pub struct WalkTheDog {
    image: Option<HtmlImageElement>,
    sheet: Option<Sheet>,
    frame: u8,
    position: Point,
}

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        let sheet = browser::fetch_json("rhb.json").await?.into_serde()?;
        let image = Some(engine::load_image("rhb.png").await?);
        Ok(Box::new(WalkTheDog {
            image,
            sheet,
            frame: self.frame,
            position: self.position,
        }))
    }
    fn update(&mut self, keystate: &KeyState) {
        if self.frame < 23 {
            self.frame += 1;
        } else {
            self.frame = 0;
        }

        let mut velocity = self.position.clone();

        if keystate.is_pressed("ArrowDown") {
            velocity.y += 3;
        }

        if keystate.is_pressed("ArrowUp") {
            velocity.y -= 3;
        }

        if keystate.is_pressed("ArrowLeft") {
            velocity.x -= 3;
        }
        if keystate.is_pressed("ArrowRight") {
            velocity.x += 3;
        }

        self.position.x = velocity.x;
        self.position.y = velocity.y;
    }
    fn draw(&self, renderer: &Renderer) {
        let current_sprite = (self.frame / 3) + 1;
        // log!("{}", current_sprite);
        let frame_name = format!("Run ({}).png", current_sprite);
        // log!("{}",frame_name);
        let sprite = self
            .sheet
            .as_ref()
            .and_then(|sheet| sheet.frames.get(&frame_name))
            .expect("Cell not found");
        renderer.clear(&Rect {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 600.0,
        });

        self.image.as_ref().map(|image| {
            renderer
                .draw_image(
                    &image,
                    &Rect {
                        x: sprite.frame.x.into(),
                        y: sprite.frame.y.into(),
                        width: sprite.frame.w.into(),
                        height: sprite.frame.h.into(),
                    },
                    &Rect {
                        x: self.position.x.into(),
                        y: self.position.y.into(),
                        width: sprite.frame.w.into(),
                        height: sprite.frame.h.into(),
                    },
                )
                .expect("Expected to draw Image");
        });
    }
}

impl WalkTheDog {
    pub fn new() -> Self {
        WalkTheDog {
            image: None,
            sheet: None,
            frame: 0,
            position: Point { x: 0, y: 0 },
        }
    }
}

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    browser::spawn_local(async move {
        let game = WalkTheDog::new();
        GameLoop::start(game)
            .await
            .expect("Could not start game loop");
    });
    Ok(())
}
