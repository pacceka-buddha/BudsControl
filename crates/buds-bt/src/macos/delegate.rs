//! The Objective-C object IOBluetooth calls back: RFCOMM channel events and the page (`openConnection`)
//! completion. All callbacks arrive on the main thread.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use buds_core::platform::LinkSink;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};

/// What happened to a channel that is being opened.
#[derive(Debug)]
pub enum OpenEvent {
    /// `rfcommChannelOpenComplete:status:` with the IOReturn status (0 = success).
    Complete(i32),
    /// The channel closed before it finished opening.
    Closed,
}

#[derive(Default)]
struct Inner {
    /// Open progress (while connecting).
    open: Option<Sender<OpenEvent>>,
    /// Page answer (`connectionComplete:status:`).
    page: Option<Sender<i32>>,
    /// Where data and the close notification go once the link exists.
    sink: Option<LinkSink>,
    /// Data that arrived before the sink was attached.
    early: Vec<Vec<u8>>,
}

pub struct State {
    inner: Mutex<Inner>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the delegate has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "BudsControlDelegate"]
    #[ivars = State]
    pub struct Delegate;

    impl Delegate {
        #[unsafe(method(rfcommChannelOpenComplete:status:))]
        fn open_complete(&self, _channel: &AnyObject, status: i32) {
            if let Some(open) = &self.ivars().inner.lock().unwrap().open {
                let _ = open.send(OpenEvent::Complete(status));
            }
        }

        #[unsafe(method(rfcommChannelClosed:))]
        fn closed(&self, _channel: &AnyObject) {
            let inner = self.ivars().inner.lock().unwrap();
            if let Some(sink) = &inner.sink {
                sink.closed();
            } else if let Some(open) = &inner.open {
                let _ = open.send(OpenEvent::Closed);
            }
        }

        #[unsafe(method(rfcommChannelData:data:length:))]
        fn data(&self, _channel: &AnyObject, data: *mut c_void, length: usize) {
            if data.is_null() || length == 0 {
                return;
            }
            // SAFETY: IOBluetooth passes a buffer of `length` bytes that is valid during the call.
            let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) }.to_vec();
            let mut inner = self.ivars().inner.lock().unwrap();
            match &inner.sink {
                Some(sink) => sink.data(bytes),
                None => inner.early.push(bytes),
            }
        }

        #[unsafe(method(connectionComplete:status:))]
        fn connection_complete(&self, _device: &AnyObject, status: i32) {
            if let Some(page) = &self.ivars().inner.lock().unwrap().page {
                let _ = page.send(status);
            }
        }
    }
);

impl Delegate {
    fn with_inner(inner: Inner) -> Retained<Delegate> {
        let this = Delegate::alloc().set_ivars(State {
            inner: Mutex::new(inner),
        });
        // SAFETY: `init` on a freshly allocated NSObject subclass.
        unsafe { msg_send![super(this), init] }
    }

    /// A delegate for opening an RFCOMM channel.
    pub fn new(open: Sender<OpenEvent>) -> Retained<Delegate> {
        Delegate::with_inner(Inner {
            open: Some(open),
            ..Inner::default()
        })
    }

    /// A delegate for a page (`openConnection`).
    pub fn new_for_page(page: Sender<i32>) -> Retained<Delegate> {
        Delegate::with_inner(Inner {
            page: Some(page),
            ..Inner::default()
        })
    }

    /// The channel is open and belongs to a link now: deliver data and the close notification to it.
    pub fn attach(&self, sink: LinkSink) {
        let mut inner = self.ivars().inner.lock().unwrap();
        for bytes in inner.early.drain(..) {
            sink.data(bytes);
        }
        inner.open = None;
        inner.sink = Some(sink);
    }

    /// Stops forwarding anything (the channel is being closed on purpose).
    pub fn detach(&self) {
        let mut inner = self.ivars().inner.lock().unwrap();
        inner.sink = None;
        inner.open = None;
        inner.page = None;
    }
}
