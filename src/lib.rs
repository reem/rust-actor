#![feature(macro_rules)]
#![license = "MIT"]
#![deny(missing_doc)]
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
            .expect(format!("Actor: {}: {}: Tried to get a actor::Distributor outside of an actor thread.",
                            file!(), line!()).as_slice())
    }}
)

/// A collection of channels that is used to communicate with running Actors.
///
/// It is stored in task local storage.
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

/// Adds the register method to `Arc<Mutex<Distributor>>`
pub trait Register {
    /// Register an Actor to be managed by this Distributor.
    fn register<A, S, R>(&self, actor: A)
    where A: Actor<S, R>,
          S: Send, R: Send;
}

impl Register for Arc<Mutex<Distributor>> {
    fn register<A, S, R>(&self, actor: A)
    where A: Actor<S, R>,
          S: Send, R: Send {
        let (actor_sender, distributor_receiver) = channel();
        let (distributor_sender, actor_receiver) = channel();

        spawn(proc() { actor.start(actor_sender, actor_receiver); });

        let dist_channel = Channel {
            sender: distributor_sender,
            receiver: distributor_receiver
        };

        self.lock().channels.insert::<ChannelKey<S, R, A>, Channel<R, S>>(dist_channel);
    }
}

impl Distributor {
    /// Create a new Distributor, putting it in task local storage.
    pub fn new() -> Arc<Mutex<Distributor>> {
        let dist = Arc::new(Mutex::new(Distributor { channels: TypeMap::new() }));
        LocalDistributor.replace(Some(dist.clone()));
        dist
    }

    // Does not need mutability, but can't be safe without exclusive access.
    #[inline]
    fn direct<A, S, R>(&mut self, message: R) -> S
    where A: Actor<S, R>,
          S: Send, R: Send {
        let channel = self.channels.find::<ChannelKey<S, R, A>, Channel<R, S>>()
            .expect("Tried to send a Message to a non-existent Actor.");
        channel.sender.send(message);
        channel.receiver.recv()
    }
}

/// An Actor, which runs independently in its own thread.
pub trait Actor<Sends, Receives>: Send {
    /// Start an Actor with a Sender and Receiver.
    ///
    /// This method should be blocking, and will be run in its own thread.
    /// One way to structure this method is to iterate over the Receiver values.
    fn start(self, Sender<Sends>, Receiver<Receives>);
}

/// A message which can be sent to an Actor from any thread with a Distributor.
pub struct Message<Data: Send> {
    data: Data
}

impl<R: Send> Message<R> {
    /// Create a new Message.
    #[inline]
    pub fn new(data: R) -> Message<R> { Message { data: data } }

    /// Send this message to the indicated Actor.
    ///
    /// This method will block the current task until the other Actor
    /// sends back a Response.
    ///
    /// ## Failure
    ///
    /// Will fail if there is no such Actor registered with the
    /// Distributor or if there if called in a task with no Distributor
    /// in the task local storage.
    #[inline]
    pub fn send<A: Actor<S, R>, S: Send>(self) -> S {
        (distributor!()).lock().direct::<A, S, R>(self.data)
    }
}

/// If any Actor has to spawn a new thread internally, it should use
/// use this instead of the prelude spawn, as this spawn will also
/// put the Distributor in task local storage in the new thread, allowing
/// that thread to send messages.
///
/// ## Failure
///
/// Will fail if there is no Distributor in the task local storage
/// of the current task.
#[inline]
pub fn spawn(todo: proc(): Send) {
    use std::task;

    let new = (distributor!()).clone();
    task::spawn(proc() {
        LocalDistributor.replace(Some(new));
        todo()
    });
}

