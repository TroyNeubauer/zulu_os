use core::task::{Context, Poll};

use crate::print;
use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use futures_util::{stream::StreamExt, Stream};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    fn new() -> Self {
        let _ = SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(128));
        Self { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut Context,
    ) -> core::task::Poll<Option<Self::Item>> {
        let queue = SCANCODE_QUEUE.try_get();
        // # Safety
        // The only way to construct a `ScancodeStream` is through calling `new`,
        // which initializes `SCANCODE_QUEUE`
        //let queue = unsafe { queue.unwrap_unchecked() };
        let queue = queue.unwrap();

        if let Ok(scancode) = queue.pop() {
            //Fast path!
            return Poll::Ready(Some(scancode));
        }
        WAKER.register(cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                let _ = WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(_err) => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        match queue.push(scancode) {
            Err(_err) => crate::println!("WARNING: scancode buf full! {} ignored", scancode),
            Ok(()) => WAKER.wake(),
        }
    } else {
        crate::println!("WARNING: scancode buf not initialized");
    }
}
