#![feature(macro_rules)]
#![license = "MIT"]
//#![deny(missing_doc)]
#![deny(warnings)]

//! Crate comment goes here

extern crate typemap;

use typemap::{TypeMap, Assoc};
use std::comm::{Sender, Receiver};
use std::sync::{Arc, Mutex};

local_data_key!(LocalDistributor: Arc<Mutex<Distributor>>)

macro_rules! distributor(
    () => {{
        LocalDistributor.get()
            .expect(format!("Lithium: {}: {}: Tried to get a lithium::Distributor outside of a lithium thread.",
                            file!(), line!()).as_slice())
    }}
)

pub struct Distributor {
    // ChannelKey -> Channel
    channels: TypeMap
}

// Distributors store these.
struct Channel<S: Send, R: Send> {
    sender: Sender<S>,
    receiver: Receiver<R>
}

// Phantom type that keys Channels in a Distributor
enum ChannelKey<S: Send, R: Send, A: Actor<S, R>> {}

impl<S: Send, R: Send, A: Actor<S, R>> Assoc<Channel<R, S>> for ChannelKey<S, R, A> {}

pub trait Register {
    fn register<A, S, R>(&self, worker: A)
    where A: Actor<S, R>,
          S: Send, R: Send;
}

impl Register for Arc<Mutex<Distributor>> {
    fn register<A, S, R>(&self, worker: A)
    where A: Actor<S, R>,
          S: Send, R: Send {
        let (worker_sender, distributor_receiver) = channel();
        let (distributor_sender, worker_receiver) = channel();

        let new = self.clone();
        // Start the worker in a different thread so it blocks.
        spawn(proc() {
            LocalDistributor.replace(Some(new));
            worker.start(worker_sender, worker_receiver);
        });

        let dist_channel = Channel {
            sender: distributor_sender,
            receiver: distributor_receiver
        };

        self.lock().channels.insert::<ChannelKey<S, R, A>, Channel<R, S>>(dist_channel);
    }
}

impl Distributor {
    pub fn new() -> Arc<Mutex<Distributor>> {
        let dist = Arc::new(Mutex::new(Distributor { channels: TypeMap::new() }));
        LocalDistributor.replace(Some(dist.clone()));
        dist
    }
    // Does not need mutability, but can't be safe without exclusive access.
    fn direct<A, S, R>(&mut self, message: R) -> Option<S>
    where A: Actor<S, R>,
          S: Send, R: Send {
        match self.channels.find::<ChannelKey<S, R, A>, Channel<R, S>>() {
            Some(channel) => {
                channel.sender.send(message);
                Some(channel.receiver.recv())
            },
            None => None
        }
    }
}

pub trait Actor<Sends, Receives>: Send {
    fn start(self, Sender<Sends>, Receiver<Receives>);
}

pub struct Message<Data: Send> {
    data: Data
}

impl<R: Send> Message<R> {
    pub fn new(data: R) -> Message<R> {
        Message {
            data: data
        }
    }

    pub fn send<A: Actor<S, R>, S: Send>(self) -> Option<S> {
        (distributor!()).lock().direct::<A, S, R>(self.data)
    }
}

pub fn spawn(todo: proc(): Send) {
    let new = (distributor!()).clone();
    spawn(proc() {
        LocalDistributor.replace(Some(new));
        todo()
    });
}

