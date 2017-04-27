extern crate futures;
extern crate thread_control;

use futures::{Future, Poll, Async};
use thread_control::{make_pair, Control, Flag};

#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Managed<F> where F: Future {
    future: F,
    flag: Option<Flag>,
}

pub fn new<F>(future: F) -> (Control, Managed<F>)
    where F: Future,
{
    let (flag, control) = make_pair();
    let managed = Managed {
        future: future,
        flag: Some(flag),
    };
    (control, managed)
}

impl<F> Future for Managed<F>
    where F: Future,
{
    type Item = F::Item;
    type Error = Option<F::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let alive = self.flag.as_ref().map(Flag::is_alive);
        if alive.unwrap_or(false) {
            match self.future.poll() {
                Ok(Async::Ready(value)) => {
                    drop(self.flag.take());
                    Ok(Async::Ready(value))
                },
                Ok(Async::NotReady) => {
                    Ok(Async::NotReady)
                },
                Err(err) => {
                    self.flag.take().map(Flag::interrupt);
                    Err(Some(err))
                },
            }
        } else {
            drop(self.flag.take());
            Err(None)
        }
    }
}

pub trait ManagedExt: Future {
    fn managed(self) -> (Control, Managed<Self>)
        where Self: Sized
    {
        new(self)
    }
}

impl<F: Future> ManagedExt for F {
}

#[cfg(test)]
mod tests {
    use futures::{future, Future, Async};
    use super::ManagedExt;

    #[test]
    fn check_managed_works() {
        let (control, mut fut) = future::ok::<u8,&'static str>(1).managed();
        assert_eq!(Ok(Async::Ready(1)), fut.poll());
        assert_eq!(control.is_done(), true);
        assert_eq!(control.is_interrupted(), false);
    }

    #[test]
    fn check_managed_stopped() {
        let (control, mut fut) = future::ok::<u8,&'static str>(1).managed();
        assert_eq!(control.is_done(), false);
        control.stop();
        assert_eq!(Err(None), fut.poll());
        assert_eq!(control.is_done(), true);
        assert_eq!(control.is_interrupted(), false);
    }

    #[test]
    fn check_managed_interrupted() {
        let (control, mut fut) = future::ok::<u8,&'static str>(1).managed();
        assert_eq!(control.is_done(), false);
        control.interrupt();
        assert_eq!(Err(None), fut.poll());
        assert_eq!(control.is_done(), true);
        assert_eq!(control.is_interrupted(), true);
    }

    #[test]
    fn check_managed_error() {
        let (control, mut fut) = future::err::<u8,&'static str>("no way").managed();
        assert_eq!(control.is_done(), false);
        assert_eq!(Err(Some("no way")), fut.poll());
        assert_eq!(control.is_done(), true);
        assert_eq!(control.is_interrupted(), true);
    }

    #[test]
    fn check_managed_interrupted_error() {
        let (control, mut fut) = future::err::<u8,&'static str>("no way").managed();
        assert_eq!(control.is_done(), false);
        control.interrupt();
        assert_eq!(Err(None), fut.poll());
        assert_eq!(control.is_done(), true);
        assert_eq!(control.is_interrupted(), true);
    }

    #[test]
    fn check_managed_is_future() {
        let (_, fut) = future::ok::<u8,&'static str>(1).managed();
        let mut fut = fut.map(|x| x + 1);
        assert_eq!(Ok(Async::Ready(2)), fut.poll());
    }

    #[test]
    fn check_stop_managed_future() {
        let (control, fut) = future::ok::<u8,&'static str>(1).managed();
        let mut fut = fut.map(|x| x + 1);
        control.stop();
        assert_eq!(Err(None), fut.poll());
    }
}
