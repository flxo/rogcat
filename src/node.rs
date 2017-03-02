// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use std::process::exit;
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;

#[derive(Clone)]
pub enum Event<T> {
    Start,
    Stop,
    Message(T),
}

pub trait Node<T: Clone, A> {
    fn new(_args: A) -> Result<Box<Self>>;

    fn start(&mut self, _send: &Fn(T), _done: &Fn()) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    fn message(&mut self, message: T) -> Result<Option<T>> {
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
                                      t: Option<Vec<NodeHandle<T>>>)
                                      -> Result<NodeHandle<T>>
        where H: Send + Node<T, A> + 'static,
              A: Send + 'static
    {
        let (tx, rx) = channel();
        let tx1 = tx.clone();
        let mut node = try!(H::new(a));

        let h = ::std::thread::spawn(move || {
            let out = |c: Event<T>| {
                match t {
                    Some(ref targets) => {
                        for n in targets {
                            n.send(c.clone()).ok(); // TODO: check
                        }
                    }
                    None => (),
                }
            };

            loop {
                let msg = rx.recv().unwrap();
                match msg {
                    Event::Start => {
                        let done = || drop(tx1.send(Event::Stop).ok());
                        let send = |payload: T| out(Event::Message(payload));
                        if let Err(e) = node.start(&send, &done) {
                            println!("{}", e);
                            exit(1);
                        }
                    }
                    Event::Stop => {
                        if let Err(e) = node.stop() {
                            println!("{}", e);
                            exit(1);
                        }
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

    pub fn run(&mut self) -> Result<()> {
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
        fn new(_: ()) -> Result<Box<Self>> {
            Ok(Box::new(S))
        }
        fn start(&mut self, send: &Fn(i32), done: &Fn()) -> Result<()> {
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
        fn new(_: ()) -> Result<Box<Self>> {
            Ok(Box::new(R { n: 0 }))
        }

        fn message(&mut self, n: i32) -> Result<Option<i32>> {
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

    let o = nodes.register::<R, _>((), None);
    let r0 = nodes.register::<R, _>((), Some(vec![o.unwrap()]));
    let r1 = nodes.register::<R, _>((), None);
    nodes.register::<S, _>((), Some(vec![r0.unwrap(), r1.unwrap()])).ok();
    assert!(nodes.run().is_ok());
}
