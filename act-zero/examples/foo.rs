use std::error::Error;

use futures::executor::LocalPool;

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
trait MyActorTrait<U: Send + 'static> {
    fn do_something(&self, x: U, res: Sender<bool>);
    fn do_generic_thing<T: Send + 'static>(&self, res: Sender<T>)
    where
        Self: Sized;
}

#[act_zero]
impl<U: Send + 'static + Default> MyActorTrait<U> for MyActor {
    async fn do_something(
        &mut self,
        _x: U,
        res: Sender<bool>,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.state += 1;
        res.send(self.state >= 4).ok();
        Ok(())
    }
    async fn do_generic_thing<T: Send + 'static>(
        self: Addr<Local<MyActor>>,
        _res: Sender<T>,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.call_do_something(U::default()).await?;
        Ok(())
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
    )?
    .upcast::<dyn MyActorTrait<_>>();

    let _ = actor_ref.call_do_something(5);
    drop(actor_ref);

    pool.run();
    Ok(())
}
