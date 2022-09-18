use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    browser::{self, LoopClosure},
    game::{Cell, Sheet},
    sound::{self},
};
use anyhow::*;
use async_trait::async_trait;
use futures::channel::{
    mpsc::{unbounded, UnboundedReceiver},
    oneshot::channel,
};

use std::result::Result::Ok;
use std::sync::Mutex;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{AudioBuffer, AudioContext, CanvasRenderingContext2d, HtmlElement, HtmlImageElement};

#[async_trait(?Send)]
pub trait Game {
    async fn initialize(&self) -> Result<Box<dyn Game>>;
    fn update(&mut self, keystate: &KeyState);
    fn draw(&self, renderer: &Renderer);
}

const FRAME_SIZE: f32 = 1.0 / 60.0 * 1000.0;
pub struct GameLoop {
    last_frame: f64,
    accumulated_delta: f32,
}

type SharedLoopClosure = Rc<RefCell<Option<LoopClosure>>>;

impl GameLoop {
    pub async fn start(game: impl Game + 'static) -> Result<()> {
        let mut keyevent_receiver = prepare_input()?;
        let mut game = game.initialize().await?;

        let mut game_loop = GameLoop {
            last_frame: browser::now()?,
            accumulated_delta: 0.0,
        };

        let renderer = Renderer {
            context: browser::context()?,
        };

        let f: SharedLoopClosure = Rc::new(RefCell::new(None));
        let g = f.clone();

        let mut keystate = KeyState::new();
        *g.borrow_mut() = Some(browser::create_raf_closure(move |perf: f64| {
            process_input(&mut keystate, &mut keyevent_receiver);
            
            let frame_time = perf - game_loop.last_frame;
            game_loop.accumulated_delta += frame_time as f32;

            while game_loop.accumulated_delta > FRAME_SIZE {
                game.update(&keystate);
                game_loop.accumulated_delta -= FRAME_SIZE;
            }

            game_loop.last_frame = perf;
            game.draw(&renderer);

            if cfg!(debug_assertions) {
                unsafe {
                    draw_frame_rate(&renderer, frame_time);
                }
            }

            browser::request_animation_frame(f.borrow().as_ref().unwrap())
                .expect("Error in Requesting frame animation from browser!");
        }));

        browser::request_animation_frame(
            g.borrow()
                .as_ref()
                .ok_or_else(|| anyhow!("GameLoop: Loop is None"))?,
        )?;
        Ok(())
    }
}

#[derive(Default)]
pub struct Rect {
    pub position: Point,
    pub width: i16,
    pub height: i16,
}

pub struct Renderer {
    context: CanvasRenderingContext2d,
}

impl Renderer {
    pub fn clear(&self, rect: &Rect) {
        self.context.clear_rect(
            rect.x().into(),
            rect.y().into(),
            rect.width.into(),
            rect.height.into(),
        );
    }

    pub fn draw_image(
        &self,
        image: &HtmlImageElement,
        frame: &Rect,
        destination: &Rect,
    ) -> Result<()> {
        self.context
            .draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &image,
                frame.x().into(),
                frame.y().into(),
                frame.width.into(),
                frame.height.into(),
                destination.x().into(),
                destination.y().into(),
                destination.width.into(),
                destination.height.into(),
            )
            .expect("Drawing is throwing exception! Unrecoverable error.");
        Ok(())
    }
    pub fn draw_entire_image(&self, image: &HtmlImageElement, position: &Point) {
        self.context
            .draw_image_with_html_image_element(image, position.x.into(), position.y.into())
            .expect("Drawing is throwing exceptions! Unrecoverable error.");
    }

    #[allow(dead_code)]
    pub fn draw_text(&self, text: &str, location: &Point) -> Result<()> {
        self.context.set_font("16pt serif");
        self.context
            .fill_text(text, location.x.into(), location.y.into())
            .map_err(|err| anyhow!("Error filling text {:#?}", err))?;
        Ok(())
    }
}

pub async fn load_image(source: &str) -> Result<HtmlImageElement> {
    let image = browser::new_image()?;
    let (complete_tx, complete_rx) = channel::<Result<()>>();
    let success_tx = Rc::new(Mutex::new(Some(complete_tx)));
    let error_tx = Rc::clone(&success_tx);
    let success_callback = browser::closure_once(move || {
        if let Some(success_tx) = success_tx.lock().ok().and_then(|mut opt| opt.take()) {
            success_tx
                .send(Ok(()))
                .expect("Success Send Failed in Image Load");
        }
    });
    let error_callback: Closure<dyn FnMut(JsValue)> = browser::closure_once(move |err| {
        if let Some(error_tx) = error_tx.lock().ok().and_then(|mut opt| opt.take()) {
            error_tx
                .send(Err(anyhow!("Error Loading Image: {:#?}", err)))
                .expect("Error Send Failed in Image Load");
        }
    });
    image.set_onload(Some(success_callback.as_ref().unchecked_ref()));
    image.set_onerror(Some(error_callback.as_ref().unchecked_ref()));
    image.set_src(source);
    complete_rx.await??;
    Ok(image)
}

enum KeyPress {
    KeyUp(web_sys::KeyboardEvent),
    KeyDown(web_sys::KeyboardEvent),
}

fn prepare_input() -> Result<UnboundedReceiver<KeyPress>> {
    let (keydown_sender, keyevent_receiver) = unbounded();

    let keydown_sender = Rc::new(RefCell::new(keydown_sender));
    let keyup_receiver = Rc::clone(&keydown_sender);

    let onkeydown = browser::closure_wrap(Box::new(move |keycode: web_sys::KeyboardEvent| {
        keydown_sender
            .borrow_mut()
            .start_send(KeyPress::KeyDown(keycode))
            .expect("Error in Registering Keydown");
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let onkeyup = browser::closure_wrap(Box::new(move |keycode: web_sys::KeyboardEvent| {
        keyup_receiver
            .borrow_mut()
            .start_send(KeyPress::KeyUp(keycode))
            .expect("Error in Registering Keyup");
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

    browser::window()?.set_onkeydown(Some(onkeydown.as_ref().unchecked_ref()));
    browser::window()?.set_onkeyup(Some(onkeyup.as_ref().unchecked_ref()));
    onkeydown.forget();
    onkeyup.forget();
    Ok(keyevent_receiver)
}

fn process_input(state: &mut KeyState, keyevent_receiver: &mut UnboundedReceiver<KeyPress>) {
    loop {
        match keyevent_receiver.try_next() {
            Ok(None) => break,
            Err(_err) => break,
            Ok(Some(evt)) => match evt {
                KeyPress::KeyUp(evt) => state.set_released(&evt.code()),
                KeyPress::KeyDown(evt) => state.set_pressed(&evt.code(), evt),
            },
        };
    }
}

#[derive(Debug)]
pub struct KeyState {
    pressed_keys: HashMap<String, web_sys::KeyboardEvent>,
}
impl KeyState {
    fn new() -> Self {
        KeyState {
            pressed_keys: HashMap::new(),
        }
    }
    pub fn is_pressed(&self, code: &str) -> bool {
        self.pressed_keys.contains_key(code)
    }
    fn set_pressed(&mut self, code: &str, event: web_sys::KeyboardEvent) {
        self.pressed_keys.insert(code.into(), event);
    }
    fn set_released(&mut self, code: &str) {
        self.pressed_keys.remove(code.into());
    }
}

#[derive(Clone, Copy, Default)]
pub struct Point {
    pub x: i16,
    pub y: i16,
}

pub struct Image {
    element: HtmlImageElement,
    bounding_box: Rect,
}
impl Image {
    pub fn new(element: HtmlImageElement, position: Point) -> Self {
        let bounding_box = Rect {
            position,
            width: element.width() as i16,
            height: element.height() as i16,
        };
        Self {
            element,
            bounding_box,
        }
    }

    pub fn draw(&self, renderer: &Renderer) {
        renderer.draw_entire_image(&self.element, &self.bounding_box.position)
    }

    pub fn bounding_box(&self) -> &Rect {
        &self.bounding_box
    }

    pub fn move_horizontally(&mut self, distance: i16) {
        self.set_x(self.bounding_box.x() + distance);
    }

    pub fn right(&self) -> i16 {
        self.bounding_box.right()
    }

    pub fn set_x(&mut self, x: i16) {
        self.bounding_box.set_x(x);
    }
}

impl Rect {
    pub const fn new(position: Point, width: i16, height: i16) -> Self {
        Rect {
            position,
            width,
            height,
        }
    }

    pub const fn new_from_x_y(x: i16, y: i16, width: i16, height: i16) -> Self {
        Rect::new(Point { x, y }, width, height)
    }

    pub fn intersects(&self, rect: &Rect) -> bool {
        self.x() < rect.right()
            && self.right() > rect.x()
            && self.y() < rect.bottom()
            && self.bottom() > rect.y()
    }

    pub fn right(&self) -> i16 {
        self.x() + self.width
    }

    pub fn bottom(&self) -> i16 {
        self.y() + self.height
    }

    pub fn x(&self) -> i16 {
        self.position.x
    }
    pub fn y(&self) -> i16 {
        self.position.y
    }

    pub fn set_x(&mut self, x: i16) {
        self.position.x = x;
    }

    pub fn set_y(&mut self, y: i16) {
        self.position.y = y;
    }
}

pub struct SpriteSheet {
    sheet: Sheet,
    image: HtmlImageElement,
}

impl SpriteSheet {
    pub fn new(sheet: Sheet, image: HtmlImageElement) -> Self {
        SpriteSheet { sheet, image }
    }

    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.sheet.frames.get(name)
    }

    pub fn draw(&self, renderer: &Renderer, source: &Rect, destination: &Rect) {
        renderer
            .draw_image(&self.image, source, destination)
            .expect("Failed to Render Sprite Sheet.");
    }
}

#[derive(Clone)]
pub struct Audio {
    context: AudioContext,
}
#[derive(Clone)]
pub struct Sound {
    pub buffer: AudioBuffer,
}

impl Audio {
    pub fn new() -> Result<Self> {
        Ok(Audio {
            context: sound::create_audio_context()?,
        })
    }

    pub async fn load_sound(&self, filename: &str) -> Result<Sound> {
        let array_buffer = browser::fetch_array_buffer(filename).await?;

        let audio_buffer = sound::decode_audio_data(&self.context, &array_buffer).await?;

        Ok(Sound {
            buffer: audio_buffer,
        })
    }

    pub fn play_sound(&self, sound: &Sound) -> Result<()> {
        sound::play_sound(&self.context, &sound.buffer, sound::LOOPING::NO)
    }

    pub fn play_looping_sound(&self, sound: &Sound) -> Result<()> {
        sound::play_sound(&self.context, &sound.buffer, sound::LOOPING::YES)
    }
}

pub fn add_click_handler(elem: HtmlElement) -> UnboundedReceiver<()> {
    let (mut click_sender, click_receiver) = unbounded();
    let on_click = browser::closure_wrap(Box::new(move || {
        click_sender
            .start_send(())
            .expect("Failed to Send Message to event handler!!");
    }) as Box<dyn FnMut()>);
    elem.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget();
    click_receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rects_that_intersects_on_the_left() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 0, y: 10 },
            height: 100,
            width: 100,
        };
        assert_eq!(rect2.intersects(&rect1), true);
    }
    #[test]
    fn two_rects_that_intersects_on_the_top() {
        let rect1 = Rect {
            position: Point { x: 10, y: -10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 0, y: 0 },
            height: 100,
            width: 100,
        };
        assert_eq!(rect2.intersects(&rect1), true);
    }
    #[test]
    fn two_rects_that_intersects_on_the_bottom() {
        let rect1 = Rect {
            position: Point { x: 0, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 0, y: 10 },
            height: 100,
            width: 100,
        };
        assert_eq!(rect2.intersects(&rect1), true);
    }
    #[test]
    fn two_rects_that_intersects_on_the_right() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 20, y: 10 },
            height: 100,
            width: 100,
        };
        assert_eq!(rect2.intersects(&rect1), true);
    }
    #[test]
    fn two_rects_that_do_not_intersects() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 200, y: 200 },
            height: 100,
            width: 100,
        };
        assert_eq!(rect2.intersects(&rect1), false);
    }
}

unsafe fn draw_frame_rate(renderer: &Renderer, frame_time: f64) {
    static mut FRAMES_COUNTED: i32 = 0;
    static mut TOTAL_FRAME_TIME: f64 = 0.0;
    static mut FRAME_RATE: i32 = 0;
    FRAMES_COUNTED += 1;
    TOTAL_FRAME_TIME += frame_time;
    if TOTAL_FRAME_TIME > 1000.0 {
        FRAME_RATE = FRAMES_COUNTED;
        TOTAL_FRAME_TIME = 0.0;
        FRAMES_COUNTED = 0;
    }
    if let Err(err) = renderer.draw_text(
        &format!("Frame Rate {}", FRAME_RATE),
        &Point { x: 400, y: 100 },
    ) {
        panic!("Could not draw text {:#?}", err);
    }
}
