use engine::GameLoop;
use engine::Image;
use engine::KeyState;
use engine::Point;
use game::Platform;
use game::RedHatBoy;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlImageElement;

use std::collections::HashMap;

#[macro_use]
mod browser;
mod engine;
mod game;

use crate::engine::{Game, Rect, Renderer};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Deserialize, Clone)]
pub struct SheetRect {
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub frame: SheetRect,
    pub sprite_source_size: SheetRect,
}

#[derive(Deserialize, Clone)]
pub struct Sheet {
    frames: HashMap<String, Cell>,
}

pub struct Walk {
    boy: RedHatBoy,
    background: Image,
    stone: Image,
    platform: Platform,
}

pub enum WalkTheDog {
    Loading,
    Loaded(Walk),
}

const LOW_PLATFORM: i16 = 420;
const HIGH_PLATFORM: i16 = 375;

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self {
            WalkTheDog::Loading => {
                let json = browser::fetch_json("rhb.json").await?;
                let rhb = RedHatBoy::new(
                    json.into_serde::<Sheet>()?,
                    engine::load_image("rhb.png").await?,
                );
                let background = engine::load_image("BG.png").await?;
                let stone = engine::load_image("Stone.png").await?;
                let platform_sheet = browser::fetch_json("tiles.json").await?;
                let platform = Platform::new(
                    platform_sheet.into_serde::<Sheet>()?,
                    engine::load_image("tiles.png").await?,
                    Point { x: 370, y: LOW_PLATFORM },
                );
                Ok(Box::new(WalkTheDog::Loaded(Walk {
                    boy: rhb,
                    background: Image::new(background, Point { x: 0, y: 0 }),
                    stone: Image::new(stone, Point { x: 150, y: 546 }),
                    platform,
                })))
            }
            WalkTheDog::Loaded(_) => Err(anyhow!("Error: Game is already initialized!")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let WalkTheDog::Loaded(walk) = self {
            if keystate.is_pressed("ArrowDown") {
                walk.boy.slide();
            }

            if keystate.is_pressed("ArrowUp") {}

            if keystate.is_pressed("ArrowLeft") {}
            if keystate.is_pressed("ArrowRight") {
                walk.boy.run_right();
            }

            if keystate.is_pressed("Space") {
                walk.boy.jump();
            }

            walk.boy.update();
            if walk
                .boy
                .bounding_box()
                .intersects(walk.stone.bounding_box())
            {
                walk.boy.knock_out();
            }

            for bounding_box in &walk.platform.bounding_box() {
                if walk.boy.bounding_box().intersects(bounding_box) {
                    if walk.boy.velocity_y() > 0 && walk.boy.pos_y() < walk.platform.position.y {
                        walk.boy.land_on(bounding_box.y);
                    } else {
                        walk.boy.knock_out();
                    }
                }
            }
        }
    }

    fn draw(&self, renderer: &Renderer) {
        if let WalkTheDog::Loaded(walk) = self {
            walk.background.draw(renderer);
            walk.boy.draw(renderer);
            walk.stone.draw(renderer);
            walk.platform.draw(renderer);
        }
    }
}

impl WalkTheDog {
    pub fn new() -> Self {
        WalkTheDog::Loading
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
