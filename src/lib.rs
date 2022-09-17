use engine::Audio;
use engine::GameLoop;
use engine::Image;
use engine::KeyState;
use engine::Point;
use engine::Rect;
use engine::SpriteSheet;
use game::WalkTheDog;
use game::rightmost;
use game::Barrier;
use game::Obstacle;
use game::Platform;
use game::RedHatBoy;
use rand::thread_rng;
use rand::Rng;
use segments::platform_and_stone;
use segments::stone_and_platform;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use web_sys::HtmlImageElement;

use std::collections::HashMap;
use std::rc::Rc;

#[macro_use]
mod browser;
mod engine;
mod game;
mod segments;
mod sound;

use crate::engine::{Game, Renderer};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;



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
