use std::error::Error;

use futures::executor::LocalPool;

use act_zero::async_fn::Closure;
use act_zero::*;

struct MyActor {
    addr: WeakAddr<Local<Self>>,
    state: i32,
}

impl Actor for MyActor {
    type Error = Box<dyn Error + Send>;

    fn started(&mut self, addr: Addr<Local<Self>>) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        println!("Started!");
        self.addr = addr.downgrade();
        Ok(())
    }
}

impl Drop for MyActor {
    fn drop(&mut self) {
        println!("Stopped ({})!", self.state);
    }
}

#[act_zero]
trait MyActorTrait {
    fn do_something(&self, res: Sender<bool>);
    fn do_generic_thing<T: Send + 'static>(&self, res: Sender<T>)
    where
        Self: Sized;
}

// =>

// trait MyActorTrait {
//     fn do_something(&self, res: Sender<bool>);
//     fn do_generic_thing<T: Send + 'static>(&self, res: Sender<T>)
//     where
//         Self: Sized;
// }

// trait __MyActorTraitInternal: Sized + Actor {
//     fn do_something(this: &Local<Self>, res: Sender<bool>);
//     fn do_generic_thing<T: Send + 'static>(this: &Local<Self>, res: Sender<T>);
// }

// enum MyActorTraitMsg {
//     DoSomething(Sender<bool>),
// }

// impl Handle<__MyActorTraitMsg> for dyn MyActorTrait {
//     fn handle(&self, msg: __MyActorTraitMsg) {
//         match msg {
//             __MyActorTraitMsg::DoSomething(res) => self.do_something(res),
//         }
//     }
// }

// impl<R> MyActorTrait for remote::Remote<R>
// where
//     R: Handle<__MyActorTraitMsg>,
// {
//     fn do_something(&self, res: Sender<bool>) {
//         self.inner().handle(__MyActorTraitMsg::DoSomething(res));
//     }
//     fn do_generic_thing<T: Send + 'static>(&self, _res: Sender<T>)
//     where
//         Self: Sized,
//     {
//         panic!("Only object-safe methods can be proxied");
//     }
// }

// impl<A> MyActorTrait for Local<A>
// where
//     A: __MyActorTraitInternal,
// {
//     fn do_something(&self, res: Sender<bool>) {
//         A::do_something(self, res)
//     }
//     fn do_generic_thing<T: Send + 'static>(&self, res: Sender<T>)
//     where
//         Self: Sized,
//     {
//         A::do_generic_thing(self, res)
//     }
// }

trait MyActorTraitExt: AddrExt
where
    Self::Inner: MyActorTrait,
{
    fn do_something(&self, res: Sender<bool>) {
        self.with(|inner| inner.do_something(res));
    }
    fn call_do_something(&self) -> Receiver<bool> {
        let (tx, rx) = channel();
        self.do_something(tx);
        rx
    }
    fn do_generic_thing<T: Send + 'static>(&self, res: Sender<T>)
    where
        Self::Inner: Sized,
    {
        self.with(|inner| inner.do_generic_thing(res));
    }
    fn call_do_generic_thing<T: Send + 'static>(&self) -> Receiver<T>
    where
        Self::Inner: Sized,
    {
        let (tx, rx) = channel();
        self.do_generic_thing(tx);
        rx
    }
}

impl<T: AddrExt> MyActorTraitExt for T where T::Inner: MyActorTrait {}

// #[act_zero(serde)]
// impl MyActorTrait for MyActor {
//     async fn do_something(&mut self, res: Sender<bool>) -> Result<(), Box<dyn Error + Send>> {
//         res.send(true)?;
//         Ok(())
//     }
//     async fn do_generic_thing<T>(&mut self, res: Sender<T>) -> Result<(), Box<dyn Error + Send>> {
//         Ok(())
//     }
// }

// =>

impl MyActor {
    async fn do_something(&mut self, res: Sender<bool>) -> Result<(), Box<dyn Error + Send>> {
        self.state += 1;
        res.send(self.state >= 4).ok();
        Ok(())
    }
    async fn do_generic_thing<T>(&mut self, _res: Sender<T>) -> Result<(), Box<dyn Error + Send>> {
        Ok(())
    }
}

impl __MyActorTraitInternal for MyActor {
    fn do_something(this: &Local<MyActor>, res: Sender<bool>) {
        async fn inner(
            actor: &mut MyActor,
            (res,): (Sender<bool>,),
        ) -> Result<(), Box<dyn Error + Send>> {
            actor.do_something(res).await
        }
        this.send_mut(Closure::new(inner, (res,)));
    }
    fn do_generic_thing<T: Send + 'static>(this: &Local<MyActor>, res: Sender<T>) {
        async fn inner<T>(
            actor: &mut MyActor,
            (res,): (Sender<T>,),
        ) -> Result<(), Box<dyn Error + Send>> {
            actor.do_generic_thing(res).await
        }
        this.send_mut(Closure::new(inner, (res,)));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pool = LocalPool::new();

    let actor_ref = spawn(
        &pool.spawner(),
        MyActor {
            addr: Default::default(),
            state: 42,
        },
    )?;

    let _ = actor_ref.call_do_something();
    drop(actor_ref);

    pool.run();
    Ok(())
}
