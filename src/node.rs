// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::thread::JoinHandle;
use std::sync::mpsc::{channel, Sender};
use std::vec::Vec;
use super::Args;

#[derive(Clone)]
enum Command<T> {
    Start,
    Stop,
    Payload(T),
    Target(Sender<Command<T>>),
}

pub trait Handler<T: Clone> {
    fn new(_args: Args) -> Box<Self>;
    fn handle(&mut self, _message: T) -> Option<T> {
        None
    }
    fn start(&self, _send: &Fn(T), _done: &Fn()) {}
    fn stop(&self) {}
}

pub struct Node<T: Clone> {
    tx: Sender<Command<T>>,
}

impl<T> Node<T>
    where T: Clone + Send + Sync + 'static
{
    pub fn new<H>(args: &Args) -> (Node<T>, JoinHandle<()>)
        where H: Send + Handler<T> + 'static
    {
        let args = args.clone();
        let (tx, rx) = channel();
        let tx_done = tx.clone();

        let handle = ::std::thread::spawn(move || {
            let mut targets: Vec<Sender<Command<T>>> = Vec::new();
            let mut node = H::new(args);
            loop {
                let msg = rx.recv().unwrap();
                match msg {
                    Command::Start => {
                        let done = || {
                            tx_done.send(Command::Stop).ok(); // TODO: check
                        };
                        let send = |payload: T| {
                            for t in &targets {
                                t.send(Command::Payload(payload.clone())).ok(); // TODO: check
                            }
                        };
                        node.start(&send, &done);
                    }
                    Command::Stop => {
                        node.stop();
                        for t in &targets {
                            t.send(Command::Stop).ok(); // TODO: check
                        }
                        break;
                    }
                    Command::Payload(msg) => {
                        if let Some(msg) = node.handle(msg) {
                            for n in &targets {
                                n.send(Command::Payload(msg.clone())).ok(); // TODO: check
                            }
                        }
                    }
                    Command::Target(target) => {
                        targets.push(target);
                    }
                }
            }
        });

        (Node { tx: tx }, handle)
    }

    pub fn add_target(&self, target: &Self) {
        self.tx.send(Command::Target(target.tx.clone())).ok(); // TODO check
    }
}

#[derive(Default)]
pub struct Nodes<T: Clone> {
    nodes: Vec<(Sender<Command<T>>, JoinHandle<()>)>,
}

impl<T> Nodes<T>
    where T: Clone + Send + Sync + 'static
{
    pub fn add_node<H: Handler<T>>(&mut self, args: &Args) -> Node<T>
        where H: Send + Handler<T> + 'static
    {
        let (node, handle) = Node::<T>::new::<H>(args);
        self.nodes.push((node.tx.clone(), handle));
        node
    }

    pub fn run(&mut self) {
        for n in &self.nodes {
            n.0.send(Command::Start).ok(); // TODO check
        }
        while let Some(h) = self.nodes.pop() {
            h.1.join().ok(); // TODO check
        }
    }
}

#[test]
fn nodes() {
    #[derive(Clone, Default)]
    struct R;
    let mut nodes = Nodes::<R>::default();
    nodes.run();
}

#[test]
fn nodes_run() {
    struct S;

    impl Handler<i32> for S {
        fn new(_: Args) -> Box<Self> {
            Box::new(S)
        }
        fn start(&self, send: &Fn(i32), done: &Fn()) {
            for i in 0..1000 {
                send(i);
            }
            done();
        }
    }

    struct R {
        n: i32,
    }

    impl Handler<i32> for R {
        fn new(_: Args) -> Box<Self> {
            Box::new(R { n: 0 })
        }

        fn handle(&mut self, n: i32) -> Option<i32> {
            assert!(self.n == n);
            self.n = self.n + 1;
            Some(n)
        }
    }

    impl Drop for R {
        fn drop(&mut self) {
            assert!(self.n == 1000);
        }
    }

    let mut nodes = Nodes::<i32>::default();
    let args = Args::default();
    let s = nodes.add_node::<S>(&args);
    let r0 = nodes.add_node::<R>(&args);
    s.add_target(&r0);
    let r1 = nodes.add_node::<R>(&args);
    r0.add_target(&r1);
    nodes.run();
}
