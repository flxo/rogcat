// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::thread::JoinHandle;
use std::sync::mpsc::{channel, Sender};
use std::vec::Vec;

#[derive(Clone)]
pub enum Event<T> {
    Start,
    Stop,
    Message(T),
}

pub trait Node<T: Clone, A> {
    fn new(_args: A) -> Result<Box<Self>, String>;

    fn start(&self, _send: &Fn(T), _done: &Fn()) -> Result<(), String> {
        Ok(())
    }

    fn stop(&self) {}

    fn message(&mut self, message: T) -> Result<Option<T>, String> {
        Ok(Some(message))
    }
}

pub type NodeHandle<T> = Sender<Event<T>>;

#[derive(Default)]
pub struct Nodes<T: Clone> {
    nodes: Vec<(Sender<Event<T>>, JoinHandle<()>)>,
}

impl<T> Nodes<T>
    where T: Clone + Send + Sync + 'static
{
    pub fn register<H: Node<T, A>, A>(&mut self,
                                      a: A,
                                      t: Vec<NodeHandle<T>>)
                                      -> Result<NodeHandle<T>, String>
        where H: Send + Node<T, A> + 'static,
              A: Send + 'static
    {
        let (tx, rx) = channel();
        let tx1 = tx.clone();
        let mut node = try!(H::new(a));

        let h = ::std::thread::spawn(move || {
            let out = |c: Event<T>| {
                for n in &t {
                    n.send(c.clone()).ok(); // TODO: check
                }
            };

            loop {
                let msg = rx.recv().unwrap();
                match msg {
                    Event::Start => {
                        let done = || {
                            tx1.send(Event::Stop).ok(); // TODO
                        };
                        let send = |payload: T| out(Event::Message(payload));
                        if let Err(e) = node.start(&send, &done) {
                            panic!(e)
                        }
                    }
                    Event::Stop => {
                        node.stop();
                        out(Event::Stop);
                        break;
                    }
                    Event::Message(msg) => {
                        if let Ok(Some(msg)) = node.message(msg) {
                            out(Event::Message(msg.clone()));
                        }
                    }
                }
            }
        });

        self.nodes.push((tx.clone(), h));
        Ok(tx)
    }

    pub fn run(&mut self) -> Result<(), String> {
        for n in &self.nodes {
            try!(n.0.send(Event::Start).map_err(|e| format!("{:?}", e)))
        }
        while let Some(h) = self.nodes.pop() {
            try!(h.1.join().map_err(|e| format!("{:?}", e)))
        }
        Ok(())
    }
}

#[test]
fn nodes() {
    #[derive(Clone, Default)]
    struct R;
    let mut nodes = Nodes::<R>::default();
    assert!(nodes.run().is_ok());
}

#[test]
fn nodes_run() {
    struct S;

    impl Node<i32, ()> for S {
        fn new(_: ()) -> Result<Box<Self>, String> {
            Ok(Box::new(S))
        }
        fn start(&self, send: &Fn(i32), done: &Fn()) -> Result<(), String> {
            for i in 0..1000 {
                send(i);
            }
            done();
            Ok(())
        }
    }

    struct R {
        n: i32,
    }

    impl Node<i32, ()> for R {
        fn new(_: ()) -> Result<Box<Self>, String> {
            Ok(Box::new(R { n: 0 }))
        }

        fn message(&mut self, n: i32) -> Result<Option<i32>, String> {
            assert!(self.n == n);
            self.n = self.n + 1;
            Ok(Some(n))
        }
    }

    impl Drop for R {
        fn drop(&mut self) {
            assert_eq!(self.n, 1000);
        }
    }

    let mut nodes = Nodes::<i32>::default();

    let o = nodes.register::<R, _>((), vec![]);
    let r0 = nodes.register::<R, _>((), vec![o.unwrap()]);
    let r1 = nodes.register::<R, _>((), vec![]);
    nodes.register::<S, _>((), vec![r0.unwrap(), r1.unwrap()]).ok();
    assert!(nodes.run().is_ok());
}
